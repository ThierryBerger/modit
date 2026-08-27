//! The runtime: everything a scenario should not have to think about.
//!
//! Owns the link, pumps events, and hands the scenario a [`Modules`] handle.
//! `Modules` is deliberately **not** generic over the transport -- the runtime
//! task keeps the link to itself and communicates over channels, so a scenario
//! signature never mentions BLE, simulation, or `Link`.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::anyhow;
use log::{debug, error, info, warn};
use shared::proto::{Command, Descriptor, Event};
use tokio::sync::{Mutex, mpsc, oneshot};

use crate::link::{Link, LinkRx, LinkTx, ModuleEvent, ModuleId};

/// Why a scenario's wait did not produce what it asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaitError {
    /// Nothing happened within the timeout.
    Timeout,
    /// A module went away. Whether that ends the scenario is the scenario's
    /// decision -- whack-a-mole could carry on with the rest, Simon Says cannot.
    ModuleLost(ModuleId),
    /// The runtime is shutting down.
    Closed,
}

impl std::fmt::Display for WaitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout => write!(f, "timed out"),
            Self::ModuleLost(id) => write!(f, "module {id} was lost"),
            Self::Closed => write!(f, "the runtime is shutting down"),
        }
    }
}

impl std::error::Error for WaitError {}

/// What a scenario is given.
///
/// Concrete on purpose: no type parameters leak into scenario code.
pub struct Modules {
    ids: Vec<ModuleId>,
    descriptors: HashMap<ModuleId, Descriptor>,
    events: Mutex<mpsc::UnboundedReceiver<(ModuleId, ModuleEvent)>>,
    commands: mpsc::UnboundedSender<(ModuleId, Command, oneshot::Sender<anyhow::Result<()>>)>,
}

impl Modules {
    /// Every module, in the order the scenario asked for them.
    pub fn ids(&self) -> &[ModuleId] {
        &self.ids
    }

    /// What a module said it was, when it connected.
    pub fn descriptor(&self, id: &ModuleId) -> Option<&Descriptor> {
        self.descriptors.get(id)
    }

    /// The next event from **any** module.
    ///
    /// This is the primitive. `wait_for_press` is a filter over it, not the
    /// other way round: a scenario that needs "the next press, whichever module"
    /// -- Simon Says, say -- cannot be built from a per-module wait, while the
    /// per-module wait is trivially built from this.
    pub async fn next_event(&self, timeout: Duration) -> Result<(ModuleId, Event), WaitError> {
        let mut events = self.events.lock().await;
        match tokio::time::timeout(timeout, events.recv()).await {
            Err(_elapsed) => Err(WaitError::Timeout),
            Ok(None) => Err(WaitError::Closed),
            Ok(Some((id, ModuleEvent::Lost))) => Err(WaitError::ModuleLost(id)),
            Ok(Some((id, ModuleEvent::Message(event)))) => Ok((id, event)),
        }
    }

    /// The next **press** from any module, skipping everything else.
    ///
    /// Prefer this to [`Self::next_event`] unless a scenario genuinely cares
    /// about other event types. Filtering by hand is easy to get subtly wrong: a
    /// `match` arm that logs and ignores a non-press still *consumes the
    /// caller's iteration*, so a queued `Hello` silently skips a step. That was
    /// a real bug in Simon Says, found by a test that went flaky because of it.
    pub async fn next_press(&self, timeout: Duration) -> Result<(ModuleId, u8), WaitError> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(WaitError::Timeout);
            }
            match self.next_event(remaining).await? {
                (from, Event::Pressed { channel }) => return Ok((from, channel)),
                (from, event) => debug!("ignoring {event:?} from {from}"),
            }
        }
    }

    /// Wait for a press on one particular module, ignoring presses elsewhere.
    ///
    /// Returns the channel that was pressed. A filter over [`Self::next_press`],
    /// which is itself a filter over [`Self::next_event`].
    pub async fn wait_for_press(&self, id: &ModuleId, timeout: Duration) -> Result<u8, WaitError> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(WaitError::Timeout);
            }
            match self.next_press(remaining).await? {
                (from, channel) if &from == id => return Ok(channel),
                (from, _) => debug!("ignoring a press on {from} while waiting on {id}"),
            }
        }
    }

    /// Drive one of a module's outputs.
    pub async fn set_output(&self, id: &ModuleId, channel: u8, on: bool) -> anyhow::Result<()> {
        self.command(id, Command::SetOutput { channel, on }).await
    }

    /// Turn every output off, on every module.
    pub async fn reset_all(&self) -> anyhow::Result<()> {
        for id in &self.ids {
            self.command(id, Command::Reset).await?;
        }
        Ok(())
    }

    async fn command(&self, id: &ModuleId, command: Command) -> anyhow::Result<()> {
        let (ack_tx, ack_rx) = oneshot::channel();
        self.commands
            .send((id.clone(), command, ack_tx))
            .map_err(|_| anyhow!("the runtime stopped before the command could be sent"))?;
        ack_rx
            .await
            .map_err(|_| anyhow!("the runtime dropped the command without answering"))?
    }
}

/// Acquire the modules, run a scenario **once**, and return its result.
///
/// Useful on its own for a scenario with a natural end, and it is what [`run`]
/// loops over.
pub async fn run_once<L, S, F>(link: &mut L, scenario: S) -> anyhow::Result<()>
where
    L: Link,
    S: FnOnce(std::sync::Arc<Modules>) -> F,
    F: std::future::Future<Output = anyhow::Result<()>>,
{
    let (ids, tx, mut rx) = link.acquire().await?;
    info!("all {} module(s) bound, starting the scenario", ids.len());

    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let (command_tx, mut command_rx) =
        mpsc::unbounded_channel::<(ModuleId, Command, oneshot::Sender<anyhow::Result<()>>)>();

    let modules = std::sync::Arc::new(Modules {
        ids,
        descriptors: HashMap::new(),
        events: Mutex::new(event_rx),
        commands: command_tx,
    });

    // One task owns the link. Sending and receiving are separate halves, so both
    // can sit in the same select! without borrowing the same value.
    let pump = tokio::spawn(async move {
        loop {
            tokio::select! {
                incoming = rx.recv() => {
                    let Some(item) = incoming else {
                        debug!("link finished");
                        break;
                    };
                    if event_tx.send(item).is_err() {
                        debug!("scenario stopped listening");
                        break;
                    }
                }
                outgoing = command_rx.recv() => {
                    let Some((id, command, ack)) = outgoing else {
                        debug!("scenario stopped sending");
                        break;
                    };
                    let result = tx.send(&id, command).await;
                    let _ = ack.send(result);
                }
            }
        }
    });

    let outcome = scenario(modules.clone()).await;
    drop(modules);
    pump.abort();
    let _ = pump.await;
    outcome
}

/// How long to wait before starting the next round.
///
/// Not just politeness: a scenario that fails *immediately* -- a capability
/// check that cannot pass, say -- would otherwise spin this loop as fast as the
/// CPU allows. The BLE link hides that behind its own acquisition backoff; the
/// simulated one does not, which is how it was noticed.
const RESTART_DELAY: Duration = Duration::from_secs(1);

/// Run a scenario against a transport, re-acquiring modules whenever it ends.
///
/// The scenario is an `async fn` taking `&Modules`, not a trait: neither
/// whack-a-mole nor Simon Says needs a lifecycle hook, so a trait would be
/// ceremony. It can become one later without changing call sites.
pub async fn run<L, S, F>(mut link: L, mut scenario: S) -> anyhow::Result<()>
where
    L: Link,
    S: FnMut(std::sync::Arc<Modules>) -> F,
    F: std::future::Future<Output = anyhow::Result<()>>,
{
    loop {
        match run_once(&mut link, &mut scenario).await {
            Ok(()) => info!("scenario finished, re-acquiring modules"),
            Err(err) => warn!("scenario ended: {err:#}; re-acquiring modules"),
        }
        tokio::time::sleep(RESTART_DELAY).await;
    }
}

/// Check that every module is running compatible firmware and has the channels
/// the scenario intends to use.
///
/// Called by a scenario at the top, so a mismatch is reported before the game
/// starts rather than as a command that silently does nothing.
pub fn require(modules: &Modules, id: &ModuleId, inputs: u8, outputs: u8) -> anyhow::Result<()> {
    let Some(descriptor) = modules.descriptor(id) else {
        // Nothing said Hello. Older firmware, or a link that does not carry
        // descriptors; not fatal on its own.
        debug!("no descriptor for {id}, skipping the capability check");
        return Ok(());
    };
    if !descriptor.is_compatible() {
        error!(
            "module {id} speaks protocol v{}, this brain speaks v{}",
            descriptor.protocol,
            shared::proto::PROTOCOL_VERSION
        );
        return Err(anyhow!("module {id} is running incompatible firmware"));
    }
    if descriptor.inputs < inputs || descriptor.outputs < outputs {
        return Err(anyhow!(
            "module {id} has {} input(s) and {} output(s), but the scenario needs \
             {inputs} and {outputs}",
            descriptor.inputs,
            descriptor.outputs
        ));
    }
    Ok(())
}
