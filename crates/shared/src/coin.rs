//! What a burst of coin-acceptor pulses is, in time.
//!
//! Every coin firmware, whatever the board, feeds these to
//! [`EdgeLatch`](crate::input::EdgeLatch), so that no two of them can disagree
//! about where one coin ends and the next begins.

/// Shortest gap between two edges that can be two genuine pulses.
///
/// Anything faster is contact bounce or noise coupled in from the acceptor's
/// solenoid, which draws a 350 mA spike when it fires. Well below the ~30 ms
/// minimum pulse the acceptor is capable of emitting, so it cannot swallow a
/// real one.
pub const PULSE_DEBOUNCE_MS: u32 = 8;

/// Quiet time that marks the end of one coin's burst.
///
/// Chosen from the two numbers on the acceptor's datasheet:
///
/// - Pulses *within* one burst are at most ~100 ms apart, even on the slowest
///   pulse-width setting, so this must be comfortably above that.
/// - Recognition takes up to 0.6 s per coin, so two coins cannot produce bursts
///   closer together than that, and this must be comfortably below it.
///
/// 300 ms sits between the two with room on both sides. If you change the
/// acceptor's pulse-speed switch, re-check it against this.
pub const BURST_GAP_MS: u32 = 300;

// `allow(dead_code)`: only the asserts below read these, which some toolchains
// do not count as a use.

/// Longest gap between two pulses of one burst, from the datasheet.
#[allow(dead_code)]
const INTRA_BURST_MS: u32 = 100;
/// Shortest time between two coins' bursts: the datasheet's recognition time.
#[allow(dead_code)]
const RECOGNITION_MS: u32 = 600;

const _: () = assert!(PULSE_DEBOUNCE_MS < INTRA_BURST_MS);
const _: () = assert!(INTRA_BURST_MS < BURST_GAP_MS && BURST_GAP_MS < RECOGNITION_MS);
