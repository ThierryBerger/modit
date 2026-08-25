mod ble;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use anyhow::Context;
use ble::*;
use btleplug::api::{Characteristic, Manager as _, Peripheral as _};
use btleplug::platform::{Manager, Peripheral};
use futures::future::join_all;
use log::{debug, error, info, warn};
use shared::{Notifier, Writable};
use tokio::{sync::Mutex, task};

use rand::RngCore;
use rand::SeedableRng;
use rand::rngs::SmallRng;

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
            service: "937312e0-2354-11eb-9f10-fbc30a62cf30",
            charac_notify_id: "917312e0-2354-11eb-9f10-fbc30a62cf30",
        },
        led: Writable {
            service: "937312e0-2354-11eb-9f10-fbc30a62cf30",
            charac_write_id: "927312e0-2354-11eb-9f10-fbc30a62cf30",
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

    'session: loop {
        let details = init_bluetooth(&adapter_list, &modules).await?;

        let missing = unbound_labels(&modules, &details);
        if !missing.is_empty() {
            warn!(
                "waiting for {} module(s): {}; rescanning",
                missing.len(),
                missing.join(", ")
            );
            continue 'session;
        }
        let buttons = details.into_iter().flatten().collect::<Vec<_>>();
        info!("all {} module(s) bound, starting the game", buttons.len());

        // reset all buttons
        for (i, button) in buttons.iter().enumerate() {
            // TODO(plan-04): a module that left between the scan and this write
            // should be demoted and re-acquired, not abort the round.
            if let Err(err) = write::write(&button.peripheral, &button.module.led, &[0]).await {
                warn!("could not reset the LED on module {i}, restarting the round: {err}");
                continue 'session;
            }
        }

        // light up random button
        let mut rng = SmallRng::seed_from_u64(42);
        let random_sleep_amount = rng.next_u64() % 1500 + 1000;
        tokio::time::sleep(std::time::Duration::from_millis(random_sleep_amount)).await;
        let rand_button_index = rng.next_u64() % buttons.len() as u64;
        let details = &buttons[rand_button_index as usize];
        if let Err(err) = write::write(&details.peripheral, &details.module.led, &[1]).await {
            warn!("could not light module {rand_button_index}, restarting the round: {err}");
            continue 'session;
        }
        info!("module {rand_button_index} is lit");

        let expected_button = Arc::new(Mutex::new(Some(rand_button_index)));
        let rng = Arc::new(Mutex::new(rng));

        let mut tasks = Vec::new();
        let should_abort = Arc::new(AtomicBool::new(false));
        for (i, button) in buttons.iter().cloned().enumerate() {
            let rng = rng.clone();
            let buttons = buttons.clone();
            let expected_button = expected_button.clone();
            let should_abort = should_abort.clone();
            let task = task::spawn(async move {
                loop {
                    if !is_connected(&button.peripheral).await {
                        should_abort.store(true, std::sync::atomic::Ordering::Relaxed);
                        warn!("module {i} disconnected, restarting the round");
                    }
                    if should_abort.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                    if let Some(value) = read::read_notification(&button.peripheral).await {
                        let Some(expected_button_index) = *expected_button.lock().await else {
                            debug!("module {i} pressed, but no module is armed yet");
                            continue;
                        };
                        if i != expected_button_index as usize {
                            info!("module {i} pressed, but {expected_button_index} was expected");
                            continue;
                        }

                        info!("module {i} hit ({:?})", value.value);
                        if let Err(err) =
                            write::write(&button.peripheral, &button.module.led, &[0]).await
                        {
                            // TODO(plan-04): demote this module and carry on
                            // rather than restarting the whole round.
                            error!("could not turn off the LED on module {i}: {err}");
                            should_abort.store(true, std::sync::atomic::Ordering::Relaxed);
                            break;
                        }

                        *expected_button.lock().await = None;
                        let rng = rng.clone();
                        let buttons = buttons.clone();
                        let expected_button_captured = expected_button.clone();
                        let should_abort = should_abort.clone();
                        task::spawn(async move {
                            // Take the values we need, then release the lock:
                            // holding it across the sleep below stalls every
                            // other task for the whole delay.
                            let (random_sleep_amount, rand_button_index) = {
                                let mut rng = rng.lock().await;
                                (
                                    rng.next_u64() % 500 + 500,
                                    rng.next_u64() % buttons.len() as u64,
                                )
                            };
                            tokio::time::sleep(std::time::Duration::from_millis(
                                random_sleep_amount,
                            ))
                            .await;
                            let details = &buttons[rand_button_index as usize];
                            if let Err(err) =
                                write::write(&details.peripheral, &details.module.led, &[1]).await
                            {
                                error!("could not light module {rand_button_index}: {err}");
                                should_abort.store(true, std::sync::atomic::Ordering::Relaxed);
                                return;
                            }
                            info!("module {rand_button_index} is lit");
                            *expected_button_captured.lock().await = Some(rand_button_index);
                        });
                    }
                }
            });
            tasks.push(task);
        }

        for (i, result) in join_all(tasks).await.into_iter().enumerate() {
            if let Err(err) = result {
                error!("the task watching module {i} did not exit cleanly: {err}");
            }
        }
    }
}
