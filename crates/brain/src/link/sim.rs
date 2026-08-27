//! A link with no radio.
//!
//! Two uses, and both matter:
//!
//! - `--simulate` runs a scenario on a laptop with no boards, driven from the
//!   keyboard. It is what makes this project re-enterable without hardware.
//! - Tests drive it programmatically, which is how scenario logic gets checked
//!   in CI.
//!
//! It carries exactly the same [`Command`]/[`Event`] values the radio would, so
//! a scenario cannot tell the difference -- which is the point.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use log::info;
use shared::proto::{Command, Descriptor, Event, PROTOCOL_VERSION, Role};
use tokio::sync::mpsc;

use super::{Link, LinkRx, LinkTx, ModuleEvent, ModuleId};

/// The event sender, in a slot so it can be swapped on re-acquisition without
/// invalidating any [`SimHandle`] already handed out.
type EventSlot = Arc<Mutex<Option<mpsc::UnboundedSender<(ModuleId, ModuleEvent)>>>>;

/// A fake module's state, so the simulator can answer `Describe` and report what
/// its outputs are doing.
#[derive(Debug, Clone)]
struct SimModule {
    descriptor: Descriptor,
    outputs: Vec<bool>,
}

impl SimModule {
    fn button(inputs: u8, outputs: u8) -> Self {
        Self {
            descriptor: Descriptor {
                protocol: PROTOCOL_VERSION,
                role: Role::Button,
                inputs,
                outputs,
            },
            outputs: vec![false; outputs as usize],
        }
    }
}

/// Shared state between the link and whoever is driving it.
///
/// Created up front by [`SimLink::new`] and valid for the life of the link,
/// including across re-acquisitions -- the sender inside is swapped rather than
/// the handle being replaced.
#[derive(Clone)]
pub struct SimHandle {
    modules: Arc<Mutex<HashMap<ModuleId, SimModule>>>,
    events: EventSlot,
    /// Every command the simulated modules have received, in order.
    ///
    /// Tests assert on this rather than polling for LED state. A blink is
    /// *transient* -- whether a poll catches it depends on scheduling, which is
    /// how a test goes flaky. The history is not transient.
    history: Arc<Mutex<Vec<(ModuleId, Command)>>>,
}

impl SimHandle {
    fn emit(&self, id: &ModuleId, event: ModuleEvent) {
        if let Some(tx) = self.events.lock().unwrap().as_ref() {
            let _ = tx.send((id.clone(), event));
        }
    }
}

impl SimHandle {
    /// Pretend someone pressed a button.
    pub fn press(&self, id: &ModuleId, channel: u8) {
        self.emit(id, ModuleEvent::Message(Event::Pressed { channel }));
    }

    /// Pretend a module lost power or went out of range.
    pub fn lose(&self, id: &ModuleId) {
        self.emit(id, ModuleEvent::Lost);
    }

    /// Everything the modules have been told to do, in order.
    #[cfg(test)]
    pub fn history(&self) -> Vec<(ModuleId, Command)> {
        self.history.lock().unwrap().clone()
    }

    /// Whether an output is currently on -- i.e. is that LED lit?
    #[cfg(test)]
    pub fn output(&self, id: &ModuleId, channel: u8) -> Option<bool> {
        let modules = self.modules.lock().unwrap();
        modules.get(id)?.outputs.get(channel as usize).copied()
    }

    /// The module whose output `channel` is currently on, if exactly one is.
    ///
    /// Scenario tests are usually asking "which one is lit?", and asking it this
    /// way makes a test fail loudly if two are.
    #[cfg(test)]
    pub fn only_lit(&self, channel: u8) -> Option<ModuleId> {
        let modules = self.modules.lock().unwrap();
        let mut lit = modules
            .iter()
            .filter(|(_, m)| m.outputs.get(channel as usize).copied().unwrap_or(false))
            .map(|(id, _)| id.clone());
        let first = lit.next()?;
        if lit.next().is_some() {
            return None;
        }
        Some(first)
    }
}

#[derive(Clone)]
pub struct SimTx {
    handle: SimHandle,
    /// Print what the outputs are doing. On for `--simulate`, off in tests.
    verbose: bool,
}

impl LinkTx for SimTx {
    async fn send(&self, id: &ModuleId, command: Command) -> anyhow::Result<()> {
        self.handle
            .history
            .lock()
            .unwrap()
            .push((id.clone(), command.clone()));

        let descriptor = {
            let mut modules = self.handle.modules.lock().unwrap();
            let Some(module) = modules.get_mut(id) else {
                anyhow::bail!("no simulated module {id}");
            };
            match command {
                Command::SetOutput { channel, on } => {
                    let Some(slot) = module.outputs.get_mut(channel as usize) else {
                        anyhow::bail!(
                            "module {id} has {} output(s), cannot set channel {channel}",
                            module.outputs.len()
                        );
                    };
                    *slot = on;
                    if self.verbose {
                        info!(
                            "[sim] {id} output {channel} -> {}",
                            if on { "ON" } else { "off" }
                        );
                    }
                    None
                }
                Command::Reset => {
                    module.outputs.iter_mut().for_each(|o| *o = false);
                    if self.verbose {
                        info!("[sim] {id} reset");
                    }
                    None
                }
                Command::Describe => Some(module.descriptor),
            }
        };
        if let Some(descriptor) = descriptor {
            self.handle
                .emit(id, ModuleEvent::Message(Event::Hello(descriptor)));
        }
        Ok(())
    }
}

pub struct SimRx {
    events: mpsc::UnboundedReceiver<(ModuleId, ModuleEvent)>,
}

impl LinkRx for SimRx {
    async fn recv(&mut self) -> Option<(ModuleId, ModuleEvent)> {
        self.events.recv().await
    }
}

/// A link over in-process channels.
pub struct SimLink {
    ids: Vec<ModuleId>,
    handle: SimHandle,
    verbose: bool,
}

impl SimLink {
    /// `ids` are the module names a scenario expects, e.g. `["a", "b"]`.
    ///
    /// Returns the handle alongside the link so a driver or a test can reach the
    /// simulated boards before the first `acquire`.
    pub fn new(ids: Vec<ModuleId>, inputs: u8, outputs: u8) -> (Self, SimHandle) {
        let modules: HashMap<_, _> = ids
            .iter()
            .map(|id| (id.clone(), SimModule::button(inputs, outputs)))
            .collect();
        let handle = SimHandle {
            modules: Arc::new(Mutex::new(modules)),
            events: Arc::new(Mutex::new(None)),
            history: Arc::new(Mutex::new(Vec::new())),
        };
        let link = Self {
            ids,
            handle: handle.clone(),
            verbose: true,
        };
        (link, handle)
    }

    /// Quieten the output. Tests do not want a log line per LED change.
    #[cfg(test)]
    pub fn quiet(mut self) -> Self {
        self.verbose = false;
        self
    }
}

impl Link for SimLink {
    type Tx = SimTx;
    type Rx = SimRx;

    async fn acquire(&mut self) -> anyhow::Result<(Vec<ModuleId>, Self::Tx, Self::Rx)> {
        // A fresh channel per acquisition, so a re-acquire starts clean. The
        // handle keeps working because it holds the *slot*, not the sender.
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        *self.handle.events.lock().unwrap() = Some(event_tx);

        // Outputs go back to their boot state, as a real reconnect would.
        for module in self.handle.modules.lock().unwrap().values_mut() {
            module.outputs.iter_mut().for_each(|o| *o = false);
        }

        // Every module greets on connect, exactly as the real ones will.
        for id in &self.ids {
            let descriptor = self.handle.modules.lock().unwrap()[id].descriptor;
            self.handle
                .emit(id, ModuleEvent::Message(Event::Hello(descriptor)));
        }

        Ok((
            self.ids.clone(),
            SimTx {
                handle: self.handle.clone(),
                verbose: self.verbose,
            },
            SimRx { events: event_rx },
        ))
    }
}
