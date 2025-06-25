#![no_std]

use serde_derive::{Deserialize, Serialize};

/// Definition of a Notifier module.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Notifier {
    /// Service uuid which is advertised
    // TODO: use proper type
    pub service: &'static str,
    /// Characteristic uuid which a client should subscribe to in order to get notifications.
    /// Data notified depends on the module:
    /// - It can be a single byte being the id of the button.
    /// - a string payload contained in a nfc
    /// - a detected distance...
    // TODO: use proper type
    pub charac_notify_id: &'static str,
}

/// Definition of a buttons module.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Writable {
    /// Service uuid which is advertised
    // TODO: use proper type
    pub service: &'static str,
    /// Characteristic uuid which a client can write to
    /// Data sent can be a single byte being the id of the button.
    // TODO: use proper type
    pub charac_write_id: &'static str,
}
