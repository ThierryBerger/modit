mod ble;

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, bail};
use ble::*;
use btleplug::api::{Characteristic, Manager as _, Peripheral as _, ValueNotification};
use btleplug::platform::{Manager, Peripheral};
use futures::StreamExt;
use futures::future::join_all;
use log::{debug, error, info, trace, warn};
use shared::{Notifier, Writable, uuids};
use tokio::sync::watch;
use tokio::{sync::Mutex, task};

use rand::rngs::SmallRng;
use rand::{RngCore, SeedableRng};

/// How often a watcher checks that its board is still there when nothing else
/// is happening. The notification stream ending is the fast path; this catches
/// a board that vanished without closing anything.
const LIVENESS_INTERVAL: Duration = Duration::from_secs(1);

/// First wait after a failed acquisition.
const BACKOFF_START: Duration = Duration::from_secs(3);

/// Ceiling on the acquisition backoff.
const BACKOFF_MAX: Duration = Duration::from_secs(30);

#[derive(Clone, Debug)]
pub struct ButtonLed {
    /// Which physical board this is. Must match the `MODIT_ID` it was flashed
    /// with, so that module 0 is the same box on every run.
    pub id: &'static str,
    pub button: Notifier,
    pub led: Writable,
}

#[derive(Clone, Debug)]
pub struct ButtonDetails {
    pub button: Characteristic,
    pub led: Characteristic,
}

impl Module<ButtonDetails> for ButtonLed {
    fn advertised_name(&self) -> String {
        format!("modit-button-{}", self.id)
    }
}

impl ModuleDefinition<ButtonDetails> for ButtonLed {
    fn label(&self) -> String {
        format!("button-led {}", self.id)
    }

    fn validate(&self) -> anyhow::Result<()> {
        self.button.validate().context("button")?;
        self.led.validate().context("led")?;
        Ok(())
    }

    async fn with_peripheral(
        &self,
        peripheral: &btleplug::platform::Peripheral,
    ) -> Option<ButtonDetails> {
        let button = (self.button.with_peripheral(peripheral).await)?;
        let led = (self.led.with_peripheral(peripheral).await)?;
        Some(ButtonDetails { button, led })
    }
}

async fn is_connected(peripheral: &Peripheral) -> bool {
    // edge case: https://github.com/deviceplug/btleplug/issues/277
    //
    // TODO(plan-04): the timeout is a workaround for that issue, but the
    // `Ok(false)` arm below is currently the only thing that reports a clean
    // disconnect, and it was previously discarded.
    tokio::select! {
        _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
            warn!("timed out asking {} whether it is connected", peripheral.address());
            false
        }
        result = peripheral.is_connected() => {
            match result {
                Ok(connected) => connected,
                Err(err) => {
                    warn!("could not query connection state of {}: {err}", peripheral.address());
                    false
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Default to `info` so a bare `cargo run` is useful, but let RUST_LOG win.
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();

    // One entry per physical board. The id must match what the board was
    // flashed with: `just flash a` produces the board this first entry expects.
    let module = |id| ButtonLed {
        id,
        button: Notifier {
            service: uuids::SERVICE,
            charac_notify_id: uuids::BUTTON_NOTIFY,
        },
        led: Writable {
            service: uuids::SERVICE,
            charac_write_id: uuids::LED_WRITE,
        },
    };
    let modules = vec![module("a"), module("b")];

    // Fail on a bad definition before touching the radio, so a typo is reported
    // as a typo rather than as a scan that never finds anything.
    validate_modules(&modules).context("invalid module definition")?;

    let manager = Manager::new()
        .await
        .context("could not initialise the Bluetooth manager")?;
    let adapter_list = manager
        .adapters()
        .await
        .context("could not list Bluetooth adapters")?;
    info!("{} Bluetooth adapter(s) available", adapter_list.len());

    // Acquisition backs off so a genuinely absent module does not spin the radio.
    let mut backoff = Duration::ZERO;

    'session: loop {
        if !backoff.is_zero() {
            debug!("waiting {backoff:?} before rescanning");
            tokio::time::sleep(backoff).await;
        }

        let details = init_bluetooth(&adapter_list, &modules).await?;

        let missing = unbound_labels(&modules, &details);
        if !missing.is_empty() {
            backoff = next_backoff(backoff);
            warn!(
                "waiting for {} module(s): {}; rescanning in {backoff:?}",
                missing.len(),
                missing.join(", ")
            );
            continue 'session;
        }
        let buttons = details.into_iter().flatten().collect::<Vec<_>>();
        if buttons.is_empty() {
            // `validate_modules` rejects an empty scenario, so this is
            // unreachable -- but the modulo below would divide by zero.
            bail!("no modules bound, refusing to start a round");
        }
        backoff = Duration::ZERO;
        info!("all {} module(s) bound, starting the game", buttons.len());

        // Open each notification stream exactly once, before the round starts,
        // and keep it for the whole round. Anything else drops presses.
        let mut streams = Vec::with_capacity(buttons.len());
        for (i, button) in buttons.iter().enumerate() {
            match read::notifications(&button.peripheral).await {
                Ok(stream) => streams.push(stream),
                Err(err) => {
                    backoff = next_backoff(backoff);
                    warn!("could not listen to module {i}, restarting the round: {err}");
                    continue 'session;
                }
            }
        }

        // reset all buttons
        for (i, button) in buttons.iter().enumerate() {
            if let Err(err) = write::write(&button.peripheral, &button.module.led, &[0]).await {
                backoff = next_backoff(backoff);
                warn!("could not reset the LED on module {i}, restarting the round: {err}");
                continue 'session;
            }
        }

        // Seeded from the OS, not a constant: the old `seed_from_u64(42)` made
        // every run play the identical sequence.
        let mut rng = SmallRng::from_os_rng();

        // Arm the first module.
        let (delay, first) = {
            let delay = Duration::from_millis(rng.next_u64() % 1500 + 1000);
            (delay, rng.next_u64() % buttons.len() as u64)
        };
        tokio::time::sleep(delay).await;
        let details = &buttons[first as usize];
        if let Err(err) = write::write(&details.peripheral, &details.module.led, &[1]).await {
            backoff = next_backoff(backoff);
            warn!("could not light module {first}, restarting the round: {err}");
            continue 'session;
        }
        info!("module {first} is lit");

        let expected_button = Arc::new(Mutex::new(Some(first)));
        let rng = Arc::new(Mutex::new(rng));

        // A round ends when any watcher decides it cannot continue. `watch` is
        // used rather than an AtomicBool so the watchers can *await* the signal
        // inside `select!` instead of polling for it.
        let (abort_tx, abort_rx) = watch::channel(false);
        let abort_tx = Arc::new(abort_tx);

        let mut tasks = Vec::new();
        for ((i, button), stream) in buttons.iter().cloned().enumerate().zip(streams) {
            let rng = rng.clone();
            let buttons = buttons.clone();
            let expected_button = expected_button.clone();
            let abort_tx = abort_tx.clone();
            let mut abort_rx = abort_rx.clone();

            let task = task::spawn(async move {
                let mut stream = stream;
                // Only characteristic this module notifies on; a second notifier
                // on the same board would otherwise be indistinguishable.
                let wanted = button.module.button.uuid;
                let mut liveness = tokio::time::interval(LIVENESS_INTERVAL);
                liveness.tick().await; // the first tick is immediate

                loop {
                    tokio::select! {
                        // Someone else ended the round.
                        _ = abort_rx.changed() => break,

                        // A notification arrived, or the stream ended.
                        item = stream.next() => {
                            let Some(notification) = item else {
                                warn!("module {i} stopped notifying, ending the round");
                                let _ = abort_tx.send(true);
                                break;
                            };
                            if notification.uuid != wanted {
                                trace!("module {i}: ignoring notification from {}", notification.uuid);
                                continue;
                            }
                            if !handle_press(
                                i,
                                &notification,
                                &button,
                                &buttons,
                                &expected_button,
                                &rng,
                                &abort_tx,
                            )
                            .await
                            {
                                break;
                            }
                        }

                        // Nothing has happened for a while: is the board still there?
                        _ = liveness.tick() => {
                            if !is_connected(&button.peripheral).await {
                                warn!("module {i} disconnected, ending the round");
                                let _ = abort_tx.send(true);
                                break;
                            }
                        }
                    }
                }
            });
            tasks.push(task);
        }
        drop(abort_rx);

        for (i, result) in join_all(tasks).await.into_iter().enumerate() {
            if let Err(err) = result {
                error!("the task watching module {i} did not exit cleanly: {err}");
            }
        }

        info!("round over, re-acquiring modules");
        backoff = next_backoff(backoff);
    }
}

/// Handle one press on module `i`. Returns `false` if the task should stop.
#[allow(clippy::too_many_arguments)]
async fn handle_press(
    i: usize,
    notification: &ValueNotification,
    button: &IdentifiedModule<ButtonDetails>,
    buttons: &[IdentifiedModule<ButtonDetails>],
    expected_button: &Arc<Mutex<Option<u64>>>,
    rng: &Arc<Mutex<SmallRng>>,
    abort_tx: &Arc<watch::Sender<bool>>,
) -> bool {
    let Some(expected) = *expected_button.lock().await else {
        debug!("module {i} pressed, but no module is armed yet");
        return true;
    };
    if i != expected as usize {
        info!("module {i} pressed, but {expected} was expected");
        return true;
    }

    info!("module {i} hit ({:?})", notification.value);
    if let Err(err) = write::write(&button.peripheral, &button.module.led, &[0]).await {
        error!("could not turn off the LED on module {i}, ending the round: {err}");
        let _ = abort_tx.send(true);
        return false;
    }

    *expected_button.lock().await = None;

    // Arm the next module after a delay, without blocking this watcher.
    let buttons = buttons.to_vec();
    let expected_button = expected_button.clone();
    let rng = rng.clone();
    let abort_tx = abort_tx.clone();
    task::spawn(async move {
        // Take what we need, then release the lock: holding it across the sleep
        // below would stall every other task for the whole delay.
        let (delay, next) = {
            let mut rng = rng.lock().await;
            (
                Duration::from_millis(rng.next_u64() % 500 + 500),
                rng.next_u64() % buttons.len() as u64,
            )
        };
        tokio::time::sleep(delay).await;

        let details = &buttons[next as usize];
        if let Err(err) = write::write(&details.peripheral, &details.module.led, &[1]).await {
            error!("could not light module {next}, ending the round: {err}");
            let _ = abort_tx.send(true);
            return;
        }
        info!("module {next} is lit");
        *expected_button.lock().await = Some(next);
    });

    true
}

/// Grow the wait between acquisition attempts, so an absent module does not
/// spin the radio forever. Starts at [`BACKOFF_START`], doubles, caps at
/// [`BACKOFF_MAX`].
fn next_backoff(current: Duration) -> Duration {
    if current.is_zero() {
        BACKOFF_START
    } else {
        (current * 2).min(BACKOFF_MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The firmware cannot reference `shared::uuids` -- `gatt!` parses UUIDs when
    /// it expands and only accepts string literals -- so the two are duplicated.
    /// This is what stops them drifting apart, which is exactly what happened to
    /// the old `assets/` files.
    ///
    /// Reads the firmware source rather than linking it, because that crate is
    /// built for a different target with a different toolchain.
    #[test]
    fn firmware_uuids_match_shared() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../module-button/src/main.rs");
        let src = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("cannot read the firmware source at {path}: {e}"));

        // Every `uuid: "..."` inside the gatt! block, in source order.
        let found: Vec<&str> = src
            .lines()
            .filter_map(|line| {
                let rest = line.trim().strip_prefix("uuid: \"")?;
                rest.split('"').next()
            })
            .collect();

        let expected = [uuids::SERVICE, uuids::LED_WRITE, uuids::BUTTON_NOTIFY];
        assert_eq!(
            found.len(),
            expected.len(),
            "expected {} uuid literals in the firmware, found {}: {found:?}. \
             If the gatt! block changed shape, update this test.",
            expected.len(),
            found.len()
        );
        for (found, expected) in found.iter().zip(expected) {
            assert_eq!(
                *found, expected,
                "the firmware and shared::uuids have drifted apart"
            );
        }
    }

    /// `shared` cannot check this itself: it has no UUID parser, by design.
    #[test]
    fn every_well_known_uuid_parses() {
        for (name, value) in [
            ("SERVICE", uuids::SERVICE),
            ("LED_WRITE", uuids::LED_WRITE),
            ("BUTTON_NOTIFY", uuids::BUTTON_NOTIFY),
        ] {
            uuid::Uuid::parse_str(value)
                .unwrap_or_else(|e| panic!("uuids::{name} is not a valid UUID: {e}"));
        }
    }

    #[test]
    fn backoff_starts_small_doubles_and_caps() {
        let mut d = Duration::ZERO;
        d = next_backoff(d);
        assert_eq!(d, BACKOFF_START);

        d = next_backoff(d);
        assert_eq!(d, BACKOFF_START * 2);

        // However many failures, it must settle at the cap rather than growing
        // without bound.
        for _ in 0..20 {
            d = next_backoff(d);
        }
        assert_eq!(d, BACKOFF_MAX);
    }

    #[test]
    fn backoff_never_exceeds_the_cap() {
        assert_eq!(next_backoff(BACKOFF_MAX), BACKOFF_MAX);
        assert!(next_backoff(BACKOFF_MAX - Duration::from_secs(1)) <= BACKOFF_MAX);
    }

    #[test]
    fn a_module_advertises_the_name_its_board_was_flashed_with() {
        let m = ButtonLed {
            id: "a",
            button: Notifier {
                service: uuids::SERVICE,
                charac_notify_id: uuids::BUTTON_NOTIFY,
            },
            led: Writable {
                service: uuids::SERVICE,
                charac_write_id: uuids::LED_WRITE,
            },
        };
        // Must match `modit-{MODIT_ROLE}-{MODIT_ID}` in the firmware.
        assert_eq!(m.advertised_name(), "modit-button-a");
    }
}
