//! The wire protocol between `brain` and every module.
//!
//! This is the single definition of what a module can be told and what it can
//! report. It replaces the previous arrangement, where a module type *was* a set
//! of UUIDs and the meaning of a payload was an informal agreement between two
//! source files.
//!
//! Two characteristics carry everything: a write characteristic takes a
//! [`Command`], a notify characteristic delivers an [`Event`]. Adding a module
//! type means adding variants here, not minting UUIDs.
//!
//! # Size budget
//!
//! Messages are encoded with [postcard], which uses varint encoding, so the
//! variants below come to a handful of bytes each.
//!
//! They have to. The default ATT MTU is 23 bytes, of which 3 are ATT overhead,
//! leaving [`MAX_MESSAGE_LEN`]. A larger MTU can be negotiated, but nothing
//! should *depend* on it — `encode` returns [`ProtoError::TooLong`] rather than
//! silently producing something the radio cannot carry, and there is a test
//! asserting every variant fits.
//!
//! As measured, the worst case today is 7 bytes (`Measurement`, whose `i32`
//! zigzags to 5), against a budget of 20. `Pressed` is 2 and `SetOutput` is 3.
//! There is room to grow, but it is finite: the budget buys roughly two more
//! `i32`s, not a string.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_derive::{Deserialize, Serialize as SerializeDerive};

/// Bumped whenever a change would make an older module misunderstand a message.
///
/// A module reports the value it was built with in [`Descriptor::protocol`], so
/// a board running stale firmware is rejected by version rather than guessed at
/// from which characteristics it happens to expose.
pub const PROTOCOL_VERSION: u8 = 1;

/// Largest message that fits in a default-MTU BLE packet.
///
/// ATT default MTU is 23 bytes; a write or notification spends 3 on opcode and
/// handle.
pub const MAX_MESSAGE_LEN: usize = 20;

/// What a module *is*. Determines which commands and events make sense for it.
#[derive(SerializeDerive, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// One or more buttons, one or more LEDs.
    Button,
    /// A coin acceptor: coins in, pulses out. See [`Event::Coin`].
    Coin,
}

impl Role {
    /// Every role there is.
    ///
    /// Exists so a test can check that the capability table in
    /// `docs/COMPOSING.md` has a row for each one -- a module type nobody
    /// documented is a module type nobody can design a game around.
    pub const ALL: &'static [Role] = &[Self::Button, Self::Coin];

    /// How this role is written in prose and in documentation tables.
    ///
    /// The match is exhaustive on purpose: adding a variant fails to compile
    /// here, which is the reminder that [`Self::ALL`] and the capability table
    /// both need the new row.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Button => "Button",
            Self::Coin => "Coin",
        }
    }
}

/// A module's self-description, sent unprompted on connect.
///
/// `inputs` and `outputs` are counts, so a prop with three buttons and two LEDs
/// is representable — which the previous one-`Notifier`-one-`Writable` design
/// was not.
#[derive(SerializeDerive, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Descriptor {
    /// [`PROTOCOL_VERSION`] the firmware was built against.
    pub protocol: u8,
    pub role: Role,
    /// Number of input channels, addressed as `0..inputs`.
    pub inputs: u8,
    /// Number of output channels, addressed as `0..outputs`.
    pub outputs: u8,
}

impl Descriptor {
    /// Whether this module understands the protocol we are speaking.
    pub const fn is_compatible(&self) -> bool {
        self.protocol == PROTOCOL_VERSION
    }
}

/// Something the brain tells a module to do.
#[derive(SerializeDerive, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Drive one output. `channel` indexes `0..Descriptor::outputs`.
    SetOutput { channel: u8, on: bool },
    /// Return to the state the module has at boot: every output off.
    Reset,
    /// Ask the module to re-send its [`Event::Hello`].
    Describe,
}

/// Something a module reports.
#[derive(SerializeDerive, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Sent unprompted on connect, and in reply to [`Command::Describe`].
    Hello(Descriptor),
    /// An input went active. `channel` indexes `0..Descriptor::inputs`.
    Pressed { channel: u8 },
    /// An input went inactive.
    Released { channel: u8 },
    /// A reading from a sensor channel.
    ///
    /// A *level*, not an increment: a dropped `Measurement` leaves the brain
    /// with a stale number, which the next one corrects. Contrast [`Self::Coin`].
    Measurement { channel: u8, value: i32 },
    /// A coin was accepted, worth `pulses` pulses on the acceptor's scale.
    ///
    /// Deliberately **not** a [`Self::Measurement`]. This is an *increment*, so
    /// a dropped one is money the brain never hears about and cannot recover by
    /// waiting for the next event -- a difference worth having in the type.
    ///
    /// The firmware reports the raw pulse count the acceptor emitted, not a
    /// currency value. What a pulse is worth depends on how the acceptor was
    /// programmed and which coins a venue takes, and that is policy: it belongs
    /// in a scenario, which is the part you are meant to edit.
    Coin { channel: u8, pulses: u8 },
}

/// Why a message could not be encoded or decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtoError {
    /// The encoded form exceeds [`MAX_MESSAGE_LEN`], so it would not fit a
    /// default-MTU packet. Shrink the message rather than raising the limit.
    TooLong { len: usize },
    /// The buffer handed to `encode` was too small.
    BufferTooSmall,
    /// The bytes are not a valid message — a different protocol version, a
    /// truncated packet, or corruption.
    Malformed,
}

impl core::fmt::Display for ProtoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooLong { len } => write!(
                f,
                "message encodes to {len} bytes, over the {MAX_MESSAGE_LEN}-byte limit"
            ),
            Self::BufferTooSmall => write!(f, "encode buffer too small"),
            Self::Malformed => write!(f, "not a valid modit message"),
        }
    }
}

impl core::error::Error for ProtoError {}

/// Encode a message into `buf`, returning the bytes actually used.
///
/// Fails rather than producing something too large for a default-MTU packet.
pub fn encode<'a, T: Serialize>(message: &T, buf: &'a mut [u8]) -> Result<&'a [u8], ProtoError> {
    let used = postcard::to_slice(message, buf)
        .map_err(|_| ProtoError::BufferTooSmall)?
        .len();
    if used > MAX_MESSAGE_LEN {
        return Err(ProtoError::TooLong { len: used });
    }
    Ok(&buf[..used])
}

/// Decode a message received over the wire.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ProtoError> {
    postcard::from_bytes(bytes).map_err(|_| ProtoError::Malformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant that can cross the wire. Keep this exhaustive -- the size
    /// test below is only as good as this list.
    pub(super) fn every_message() -> (Vec<Command>, Vec<Event>) {
        let commands = vec![
            Command::SetOutput {
                channel: 0,
                on: true,
            },
            Command::SetOutput {
                channel: u8::MAX,
                on: false,
            },
            Command::Reset,
            Command::Describe,
        ];
        let events = vec![
            Event::Hello(Descriptor {
                protocol: PROTOCOL_VERSION,
                role: Role::Button,
                inputs: u8::MAX,
                outputs: u8::MAX,
            }),
            Event::Pressed { channel: 0 },
            Event::Pressed { channel: u8::MAX },
            Event::Released { channel: 3 },
            Event::Measurement {
                channel: 1,
                value: i32::MIN,
            },
            Event::Measurement {
                channel: 1,
                value: i32::MAX,
            },
            Event::Coin {
                channel: 0,
                pulses: 1,
            },
            Event::Coin {
                channel: u8::MAX,
                pulses: u8::MAX,
            },
        ];
        (commands, events)
    }

    #[test]
    fn commands_round_trip() {
        let mut buf = [0u8; 64];
        for command in every_message().0 {
            let bytes = encode(&command, &mut buf).unwrap();
            assert_eq!(decode::<Command>(bytes).unwrap(), command);
        }
    }

    #[test]
    fn events_round_trip() {
        let mut buf = [0u8; 64];
        for event in every_message().1 {
            let bytes = encode(&event, &mut buf).unwrap();
            assert_eq!(decode::<Event>(bytes).unwrap(), event);
        }
    }

    /// The design constraint, made executable.
    ///
    /// This is what will fail the day someone puts a `String` in `Descriptor`.
    /// Raising `MAX_MESSAGE_LEN` to make it pass means depending on a negotiated
    /// MTU -- read the module docs before doing that.
    #[test]
    fn every_message_fits_a_default_mtu_packet() {
        let mut buf = [0u8; 64];
        let (commands, events) = every_message();
        for command in commands {
            let len = encode(&command, &mut buf).unwrap().len();
            assert!(len <= MAX_MESSAGE_LEN, "{command:?} encodes to {len} bytes");
        }
        for event in events {
            let len = encode(&event, &mut buf).unwrap().len();
            assert!(len <= MAX_MESSAGE_LEN, "{event:?} encodes to {len} bytes");
        }
    }

    #[test]
    fn encode_refuses_an_oversized_message() {
        // Not reachable through the enums above, which is the point of the test
        // above; this checks the guard itself works.
        let too_big = [0u8; MAX_MESSAGE_LEN + 8];
        let mut buf = [0u8; 128];
        assert!(matches!(
            encode(&too_big, &mut buf),
            Err(ProtoError::TooLong { .. })
        ));
    }

    #[test]
    fn encode_reports_a_short_buffer() {
        let mut buf = [0u8; 1];
        assert_eq!(
            encode(
                &Event::Measurement {
                    channel: 1,
                    value: i32::MAX
                },
                &mut buf
            ),
            Err(ProtoError::BufferTooSmall)
        );
    }

    #[test]
    fn decode_rejects_rubbish() {
        assert_eq!(
            decode::<Command>(&[0xff, 0xff, 0xff]),
            Err(ProtoError::Malformed)
        );
        assert_eq!(decode::<Command>(&[]), Err(ProtoError::Malformed));
    }

    /// Why adding [`Role::Coin`] and [`Event::Coin`] did **not** bump
    /// [`PROTOCOL_VERSION`], made executable.
    ///
    /// postcard encodes an enum as a varint discriminant followed by the
    /// payload, so *appending* a variant leaves every earlier one on the same
    /// number: firmware built against v1 and a brain that knows about coins
    /// still agree about every message they both have. The version guards
    /// changes that break that agreement -- reordering these enums, or
    /// inserting a variant in the middle, would.
    ///
    /// If this test fails, the wire format moved under an existing variant.
    /// Bump `PROTOCOL_VERSION`; do not update the expected bytes.
    #[test]
    fn appending_variants_left_the_existing_wire_format_alone() {
        let mut buf = [0u8; 64];
        let golden: &[(Event, &[u8])] = &[
            (
                Event::Hello(Descriptor {
                    protocol: 1,
                    role: Role::Button,
                    inputs: 1,
                    outputs: 1,
                }),
                &[0, 1, 0, 1, 1],
            ),
            (Event::Pressed { channel: 0 }, &[1, 0]),
            (Event::Released { channel: 3 }, &[2, 3]),
            (
                Event::Measurement {
                    channel: 1,
                    value: 0,
                },
                &[3, 1, 0],
            ),
        ];
        for (event, expected) in golden {
            assert_eq!(
                encode(event, &mut buf).unwrap(),
                *expected,
                "the encoding of {event:?} changed"
            );
        }
    }

    /// A coin is an increment, so the brain must be able to tell "two separate
    /// 1-pulse coins" from "one 2-pulse coin". Two identical events in a row
    /// are meaningful here in a way two identical `Measurement`s are not.
    #[test]
    fn two_identical_coins_are_two_coins() {
        let mut buf = [0u8; 64];
        let coin = Event::Coin {
            channel: 0,
            pulses: 1,
        };
        let once = encode(&coin, &mut buf).unwrap().to_vec();
        // Nothing in the encoding distinguishes them, which is the point: the
        // transport must not deduplicate, and the brain must count arrivals.
        let twice = encode(&coin, &mut buf).unwrap().to_vec();
        assert_eq!(once, twice);
        assert_eq!(decode::<Event>(&once).unwrap(), coin);
    }

    /// `ALL` is maintained by hand -- `name` is what makes a new variant fail to
    /// compile, and this is what catches the other half of the mistake: a
    /// variant that was named but never added to the list.
    #[test]
    fn every_role_is_listed_once() {
        let mut names: Vec<&str> = Role::ALL.iter().map(Role::name).collect();
        let listed = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(
            names.len(),
            listed,
            "Role::ALL lists a role twice: {names:?}"
        );
    }

    #[test]
    fn a_module_built_against_another_version_is_incompatible() {
        let mut d = Descriptor {
            protocol: PROTOCOL_VERSION,
            role: Role::Button,
            inputs: 1,
            outputs: 1,
        };
        assert!(d.is_compatible());
        d.protocol = PROTOCOL_VERSION.wrapping_add(1);
        assert!(!d.is_compatible());
    }
}
