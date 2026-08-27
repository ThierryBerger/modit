//! The transport seam.
//!
//! Everything above this point deals in [`ModuleId`]s and
//! [`shared::proto`] messages. Whether those travel over BLE, an in-process
//! channel, or something not written yet is not the runtime's concern.
//!
//! A [`Link`] is acquired once and then splits into two halves, because sending
//! and receiving need different access: a sender is shared and cloneable, a
//! receiver is owned and mutable. Keeping them separate is what lets the runtime
//! task hold both in one `select!`.

pub mod ble;
pub mod sim;

use std::fmt;

use shared::proto::{Command, Event};

/// Which module. Stable across reconnects -- it is the id the board was flashed
/// with, not a discovery index.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModuleId(pub String);

impl fmt::Display for ModuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for ModuleId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// What the runtime hears from a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleEvent {
    /// The module said something.
    Message(Event),
    /// The module went away. The runtime turns this into a
    /// [`WaitError::ModuleLost`](crate::runtime::WaitError::ModuleLost) for
    /// whichever scenario call is waiting.
    Lost,
}

/// Sending half. Cloneable and shared, so several tasks can command modules.
pub trait LinkTx: Clone + Send + Sync + 'static {
    fn send(
        &self,
        id: &ModuleId,
        command: Command,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;
}

/// Receiving half. Owned by exactly one task.
pub trait LinkRx: Send + 'static {
    /// The next event from any module, or `None` when the link is finished and
    /// the modules need re-acquiring.
    fn recv(&mut self)
    -> impl std::future::Future<Output = Option<(ModuleId, ModuleEvent)>> + Send;
}

/// A transport that can find modules and then talk to them.
pub trait Link {
    type Tx: LinkTx;
    type Rx: LinkRx;

    /// Find every module the scenario needs, retrying until they are all there.
    ///
    /// Returns the ids actually acquired, in the order the caller asked for
    /// them, plus the two halves of the link.
    fn acquire(
        &mut self,
    ) -> impl std::future::Future<Output = anyhow::Result<(Vec<ModuleId>, Self::Tx, Self::Rx)>>;
}
