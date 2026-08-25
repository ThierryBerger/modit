//! Types and constants shared between the host (`brain`) and the firmware
//! (`module-*`).
//!
//! `no_std` when built for a board, `std` under `cargo test` so the constants
//! below can be checked on the host.

#![cfg_attr(not(test), no_std)]

use serde_derive::{Deserialize, Serialize};

/// The BLE UUIDs every modit module speaks.
///
/// These are the single source of truth. `brain` uses them directly; the
/// firmware cannot (the `gatt!` macro parses UUIDs at expansion time and so
/// requires string *literals*), and instead asserts at compile time that its
/// literals still match — see `str_eq`.
pub mod uuids {
    /// The one service every module advertises.
    ///
    /// Modules are told apart by their advertised *name* (`modit-<role>-<id>`),
    /// not by their service UUID, so this is deliberately the same everywhere.
    pub const SERVICE: &str = "937312e0-2354-11eb-9f10-fbc30a62cf30";

    /// Characteristic a client writes to in order to drive the LED.
    ///
    /// One byte: `0` off, anything else on.
    pub const LED_WRITE: &str = "927312e0-2354-11eb-9f10-fbc30a62cf30";

    /// Characteristic a client subscribes to for button presses.
    pub const BUTTON_NOTIFY: &str = "917312e0-2354-11eb-9f10-fbc30a62cf30";
}

/// Compare two strings in a `const` context.
///
/// Exists so the firmware can assert that the literals it hands to `gatt!` still
/// match [`uuids`], turning a drift between the two into a compile error rather
/// than a module that advertises characteristics the brain never finds.
/// `str::eq` is not `const`.
pub const fn str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Definition of a Notifier module. For example a sensor, button...
#[derive(Serialize, Deserialize, Debug, Hash, Clone, PartialEq, Eq)]
pub struct Notifier {
    /// Service uuid which is advertised
    pub service: &'static str,
    /// Characteristic uuid which a client should subscribe to in order to get notifications.
    /// Data notified depends on the module:
    /// - It can be a single byte being the id of the button.
    /// - a string payload contained in a nfc
    /// - a detected distance...
    pub charac_notify_id: &'static str,
}

/// Definition of a module which can be written to. For example a motor, LED or screen...
#[derive(Serialize, Deserialize, Debug, Hash, Clone, PartialEq, Eq)]
pub struct Writable {
    /// Service uuid which is advertised
    pub service: &'static str,
    /// Characteristic uuid which a client can write to
    /// Data sent can be a single byte being the id of a particular appliance or a speed, direction...
    pub charac_write_id: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn str_eq_matches_str_equality() {
        assert!(str_eq("abc", "abc"));
        assert!(!str_eq("abc", "abd"));
        assert!(!str_eq("abc", "ab"));
        assert!(!str_eq("ab", "abc"));
        assert!(str_eq("", ""));
    }

    #[test]
    fn the_uuids_are_distinct() {
        let all = [uuids::SERVICE, uuids::LED_WRITE, uuids::BUTTON_NOTIFY];
        for (i, a) in all.iter().enumerate() {
            for b in &all[..i] {
                assert_ne!(a, b, "two well-known UUIDs collide");
            }
        }
    }
}
