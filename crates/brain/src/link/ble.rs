//! [`Link`] over BLE.
//!
//! **Deliberately thin.** Acquisition, connection and the notification streams
//! are the code in [`crate::ble`], unchanged -- this only translates between it
//! and [`shared::proto`] messages. That code has not been run against real
//! hardware since the event-loop rewrite, so keeping it untouched means a
//! misbehaving board can be blamed on one change rather than two.
//!
//! The translation is here rather than on the board because `module-button`
//! speaks a button-specific wire format -- `&[0]`/`&[1]` for the LED,
//! `b"Notification"` for a press. This file gets simpler when that firmware
//! speaks [`shared::proto`] like `module-coin` already does.

use std::time::Duration;

use btleplug::api::Peripheral as _;
use btleplug::platform::Adapter;
use futures::StreamExt;
use futures::stream::SelectAll;
use log::{debug, info, trace, warn};
use shared::proto::{Command, Descriptor, Event, PROTOCOL_VERSION, Role};

use super::{Link, LinkRx, LinkTx, ModuleEvent, ModuleId};
use crate::ble::{IdentifiedModule, init_bluetooth, unbound_labels, write};
use crate::{ButtonDetails, ButtonLed};

/// How often to check a module is still there when nothing is arriving.
const LIVENESS_INTERVAL: Duration = Duration::from_secs(1);

/// First wait after a failed acquisition.
const BACKOFF_START: Duration = Duration::from_secs(3);

/// Ceiling on the acquisition backoff.
const BACKOFF_MAX: Duration = Duration::from_secs(30);

/// Grow the wait between acquisition attempts, so an absent module does not spin
/// the radio. Starts at [`BACKOFF_START`], doubles, caps at [`BACKOFF_MAX`].
pub fn next_backoff(current: Duration) -> Duration {
    if current.is_zero() {
        BACKOFF_START
    } else {
        (current * 2).min(BACKOFF_MAX)
    }
}

#[derive(Clone)]
pub struct BleTx {
    modules: Vec<(ModuleId, IdentifiedModule<ButtonDetails>)>,
}

impl BleTx {
    fn find(&self, id: &ModuleId) -> anyhow::Result<&IdentifiedModule<ButtonDetails>> {
        self.modules
            .iter()
            .find(|(candidate, _)| candidate == id)
            .map(|(_, module)| module)
            .ok_or_else(|| anyhow::anyhow!("no bound module {id}"))
    }
}

impl LinkTx for BleTx {
    async fn send(&self, id: &ModuleId, command: Command) -> anyhow::Result<()> {
        let module = self.find(id)?;
        // The button firmware's own wire format, not `proto::encode`.
        let payload: &[u8] = match command {
            Command::SetOutput { channel: 0, on } => {
                if on {
                    &[1]
                } else {
                    &[0]
                }
            }
            Command::SetOutput { channel, .. } => {
                anyhow::bail!("module {id} has one output, cannot set channel {channel}")
            }
            Command::Reset => &[0],
            // Nothing to ask: the descriptor is synthesised in `acquire`.
            Command::Describe => return Ok(()),
        };
        write::write(&module.peripheral, &module.module.led, payload)
            .await
            .map_err(|err| anyhow::anyhow!("write to module {id} failed: {err}"))
    }
}

/// One module's notification stream, tagged with which module it is, plus a
/// liveness tick so a board that vanishes without closing anything is noticed.
struct ModuleStream {
    id: ModuleId,
    notifications: crate::ble::read::Notifications,
    peripheral: btleplug::platform::Peripheral,
    wanted: uuid::Uuid,
    liveness: tokio::time::Interval,
    lost: bool,
}

impl ModuleStream {
    async fn next(&mut self) -> Option<(ModuleId, ModuleEvent)> {
        if self.lost {
            return None;
        }
        loop {
            tokio::select! {
                item = self.notifications.next() => {
                    let Some(notification) = item else {
                        warn!("module {} stopped notifying", self.id);
                        self.lost = true;
                        return Some((self.id.clone(), ModuleEvent::Lost));
                    };
                    if notification.uuid != self.wanted {
                        trace!("module {}: ignoring {}", self.id, notification.uuid);
                        continue;
                    }
                    // Old wire format: any payload on this characteristic is a
                    // press of the module's only button.
                    return Some((
                        self.id.clone(),
                        ModuleEvent::Message(Event::Pressed { channel: 0 }),
                    ));
                }
                _ = self.liveness.tick() => {
                    if !is_connected(&self.id, &self.peripheral).await {
                        warn!("module {} disconnected", self.id);
                        self.lost = true;
                        return Some((self.id.clone(), ModuleEvent::Lost));
                    }
                }
            }
        }
    }
}

/// Ask a peripheral whether it is still there, giving up after a second.
///
/// Takes the module id purely for the log. `peripheral.address()` was used
/// before, and on macOS that prints `00:00:00:00:00:00` -- CoreBluetooth does
/// not expose MAC addresses, so btleplug identifies peripherals by UUID and the
/// address is a placeholder. A line naming the module is the one a person
/// reading a game log can act on.
async fn is_connected(id: &ModuleId, peripheral: &btleplug::platform::Peripheral) -> bool {
    // edge case: https://github.com/deviceplug/btleplug/issues/277
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(1)) => {
            warn!("timed out asking module {id} whether it is connected");
            false
        }
        result = peripheral.is_connected() => match result {
            Ok(connected) => connected,
            Err(err) => {
                warn!("could not query connection state of module {id}: {err}");
                false
            }
        },
    }
}

pub struct BleRx {
    streams: Vec<ModuleStream>,
    /// Descriptors to deliver before anything else, so the runtime sees a Hello
    /// from every module exactly as it will once the firmware sends real ones.
    pending_hellos: Vec<(ModuleId, ModuleEvent)>,
}

impl LinkRx for BleRx {
    async fn recv(&mut self) -> Option<(ModuleId, ModuleEvent)> {
        if let Some(hello) = self.pending_hellos.pop() {
            return Some(hello);
        }
        if self.streams.iter().all(|s| s.lost) {
            return None;
        }
        // Poll every module's stream concurrently and take whichever speaks
        // first.
        let mut futures: SelectAll<_> = self
            .streams
            .iter_mut()
            .filter(|s| !s.lost)
            .map(|s| Box::pin(futures::stream::once(s.next())))
            .collect();
        futures.next().await.flatten()
    }
}

/// BLE transport for a fixed set of button modules.
pub struct BleLink {
    adapters: Vec<Adapter>,
    modules: Vec<ButtonLed>,
    backoff: Duration,
}

impl BleLink {
    pub fn new(adapters: Vec<Adapter>, modules: Vec<ButtonLed>) -> Self {
        Self {
            adapters,
            modules,
            backoff: Duration::ZERO,
        }
    }
}

impl Link for BleLink {
    type Tx = BleTx;
    type Rx = BleRx;

    async fn acquire(&mut self) -> anyhow::Result<(Vec<ModuleId>, Self::Tx, Self::Rx)> {
        loop {
            if !self.backoff.is_zero() {
                debug!("waiting {:?} before rescanning", self.backoff);
                tokio::time::sleep(self.backoff).await;
            }

            let found = init_bluetooth(&self.adapters, &self.modules).await?;

            let missing = unbound_labels(&self.modules, &found);
            if !missing.is_empty() {
                self.backoff = next_backoff(self.backoff);
                warn!(
                    "waiting for {} module(s): {}; rescanning in {:?}",
                    missing.len(),
                    missing.join(", "),
                    self.backoff
                );
                continue;
            }

            let bound: Vec<_> = found.into_iter().flatten().collect();
            if bound.is_empty() {
                anyhow::bail!("no modules bound, refusing to start");
            }
            self.backoff = Duration::ZERO;

            let mut ids = Vec::with_capacity(bound.len());
            let mut tagged = Vec::with_capacity(bound.len());
            let mut streams = Vec::with_capacity(bound.len());
            let mut pending_hellos = Vec::with_capacity(bound.len());

            for (definition, module) in self.modules.iter().zip(bound) {
                let id = ModuleId(definition.id.to_string());
                let notifications = crate::ble::read::notifications(&module.peripheral)
                    .await
                    .map_err(|err| anyhow::anyhow!("cannot listen to module {id}: {err}"))?;

                let mut liveness = tokio::time::interval(LIVENESS_INTERVAL);
                liveness.tick().await; // the first tick is immediate

                streams.push(ModuleStream {
                    id: id.clone(),
                    notifications,
                    peripheral: module.peripheral.clone(),
                    wanted: module.module.button.uuid,
                    liveness,
                    lost: false,
                });

                // The button firmware sends no Hello, so synthesise the one it
                // would send. Keeping the runtime's view identical either way
                // means the capability checks are exercised against real boards
                // and not only in simulation.
                pending_hellos.push((
                    id.clone(),
                    ModuleEvent::Message(Event::Hello(Descriptor {
                        protocol: PROTOCOL_VERSION,
                        role: Role::Button,
                        inputs: 1,
                        outputs: 1,
                    })),
                ));

                ids.push(id.clone());
                tagged.push((id, module));
            }

            info!("bound {} module(s) over BLE", ids.len());
            return Ok((
                ids,
                BleTx { modules: tagged },
                BleRx {
                    streams,
                    pending_hellos,
                },
            ));
        }
    }
}
