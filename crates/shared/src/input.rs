//! Counting edges from an interrupt, without losing any.
//!
//! An input event is often much shorter than the loop that would like to notice
//! it. A coin acceptor's pulse is ~30 ms; a tennis ball in contact with a rigid
//! plate is 4-6 ms. Neither survives being sampled once per pass of a BLE work
//! loop whose period depends on what the radio is doing.
//!
//! So the pin is an interrupt, and [`EdgeLatch`] is what the interrupt handler
//! writes into and the main loop reads out of.
//!
//! # Why this lives in `shared` and not in a firmware crate
//!
//! Nothing here touches `esp_hal`, an allocator, or a clock. **Time is an
//! argument.** That is deliberate: it makes debounce and burst-settling rules
//! testable with `cargo test` on a laptop, instead of testable only by posting
//! real coins into a real slot. The firmware supplies `now_ms()` and the pin;
//! this supplies the rules.
//!
//! # Why `u32` milliseconds
//!
//! The Xtensa ESP32 core has **no 64-bit atomics**, so an `AtomicU64` timestamp
//! cannot be shared with an interrupt handler at all. `u32` milliseconds wrap
//! after ~49 days; every comparison here uses `wrapping_sub`, so a wrap costs at
//! most one mis-timed boundary rather than a hang or a stuck counter.

use core::sync::atomic::{AtomicU32, Ordering};

/// Edges counted by an interrupt handler, waiting to be read by a main loop.
///
/// Intended to live in a `static`. Every method takes `&self`, so the handler
/// and the loop can share it with no lock.
///
/// ```
/// # use shared::input::EdgeLatch;
/// static PRESSES: EdgeLatch = EdgeLatch::new();
///
/// // in the interrupt handler:
/// PRESSES.record(1_000, 50);
/// // in the main loop:
/// assert_eq!(PRESSES.take(), 1);
/// ```
#[derive(Debug)]
pub struct EdgeLatch {
    /// Edges accepted but not yet handed to the main loop.
    count: AtomicU32,
    /// When the most recently *accepted* edge arrived.
    last_ms: AtomicU32,
    /// Whether any edge has ever been accepted.
    ///
    /// Without this, "no edge yet" and "an edge at millisecond 0" are the same
    /// state, and the debounce window would reject a genuine first edge on a
    /// board whose clock happens to still read 0.
    seen: AtomicU32,
}

impl EdgeLatch {
    /// An empty latch. `const`, so it can initialise a `static`.
    pub const fn new() -> Self {
        Self {
            count: AtomicU32::new(0),
            last_ms: AtomicU32::new(0),
            seen: AtomicU32::new(0),
        }
    }

    /// Record one edge, rejecting it if it falls inside the debounce window.
    ///
    /// Returns whether the edge was counted, which is only useful for logging --
    /// callers in an interrupt handler should ignore it and get out.
    ///
    /// `debounce_ms` is a parameter rather than a field so that one type serves
    /// inputs with very different physics: 8 ms for a coin pulse, hundreds for a
    /// plate that rings after it is struck.
    pub fn record(&self, now_ms: u32, debounce_ms: u32) -> bool {
        if self.seen.load(Ordering::Relaxed) != 0 {
            let last = self.last_ms.load(Ordering::Relaxed);
            if now_ms.wrapping_sub(last) < debounce_ms {
                return false;
            }
        }
        self.count.fetch_add(1, Ordering::Relaxed);
        self.last_ms.store(now_ms, Ordering::Relaxed);
        self.seen.store(1, Ordering::Relaxed);
        true
    }

    /// Record the input going **inactive** -- the release, the end of a pulse.
    ///
    /// Counts nothing. Its only job is to restart the debounce window, so that
    /// contact bounce *on release* is not read as a second press.
    ///
    /// # Why this is needed
    ///
    /// [`Self::record`] measures the window from the last **accepted** edge, so
    /// a press that is held for longer than `debounce_ms` -- which is every
    /// press a finger makes -- leaves the window already expired by the time the
    /// button is let go. A switch bounces on release exactly as it does on
    /// press, and each of those contact closures is another rising edge. The
    /// first one lands outside the window and is counted, so **one press
    /// produces two events**, the second arriving 50-200 ms after the first.
    ///
    /// Watching the release closes that gap: the release resets the window, and
    /// the bounce that follows it falls inside.
    ///
    /// Returns whether the release was accepted, which is only useful for
    /// logging -- bounce on the release edge is itself rejected, which is the
    /// point.
    pub fn record_release(&self, now_ms: u32, debounce_ms: u32) -> bool {
        if self.seen.load(Ordering::Relaxed) != 0 {
            let last = self.last_ms.load(Ordering::Relaxed);
            if now_ms.wrapping_sub(last) < debounce_ms {
                return false;
            }
        }
        self.last_ms.store(now_ms, Ordering::Relaxed);
        // A release seen before any press still arms the window -- a board that
        // boots with the button held must not count the let-go as a press.
        self.seen.store(1, Ordering::Relaxed);
        true
    }

    /// How many edges are waiting, without consuming them.
    pub fn pending(&self) -> u32 {
        self.count.load(Ordering::Relaxed)
    }

    /// When the most recently accepted edge arrived, or `None` if there has
    /// never been one.
    pub fn last_ms(&self) -> Option<u32> {
        (self.seen.load(Ordering::Relaxed) != 0).then(|| self.last_ms.load(Ordering::Relaxed))
    }

    /// Take every edge counted so far.
    ///
    /// Subtracts exactly what it observed rather than storing zero. An edge
    /// arriving between the read and the clear then survives into the next
    /// call instead of vanishing -- for a coin acceptor that difference is
    /// money, and for an impact target it is a hit the player made and was not
    /// credited with. **Do not "simplify" this into `swap(0)`**; there is a test
    /// named after this paragraph.
    pub fn take(&self) -> u32 {
        let observed = self.count.load(Ordering::Relaxed);
        if observed > 0 {
            self.count.fetch_sub(observed, Ordering::Relaxed);
        }
        observed
    }

    /// Take exactly one edge, if any is waiting.
    ///
    /// For an input where every edge is its own event -- a button press, a hit
    /// on a target -- as opposed to [`Self::take_settled`]'s burst, where several
    /// edges together name one thing.
    ///
    /// Prefer this to `take() > 0` in a loop that can emit only one event per
    /// pass: `take` would swallow the extras, quietly reintroducing the dropped
    /// input this whole module exists to prevent.
    ///
    /// # Concurrency
    ///
    /// Assumes a **single consumer**. The interrupt handler only ever increments,
    /// so the decrement here cannot race with it; two threads calling `take_one`
    /// could both observe the same edge.
    pub fn take_one(&self) -> bool {
        if self.count.load(Ordering::Relaxed) == 0 {
            return false;
        }
        self.count.fetch_sub(1, Ordering::Relaxed);
        true
    }

    /// Take a burst of edges, but only once it has stopped growing.
    ///
    /// Returns `None` while edges are still arriving, so a three-pulse coin is
    /// reported once as `3` rather than three times as `1`. `gap_ms` is the
    /// quiet time that marks the end of a burst.
    pub fn take_settled(&self, now_ms: u32, gap_ms: u32) -> Option<u32> {
        if self.count.load(Ordering::Relaxed) == 0 {
            return None;
        }
        let last = self.last_ms.load(Ordering::Relaxed);
        if now_ms.wrapping_sub(last) < gap_ms {
            return None;
        }
        let taken = self.take();
        (taken > 0).then_some(taken)
    }

    /// Throw away everything uncounted, returning how much was discarded.
    ///
    /// What [`Command::Reset`](crate::proto::Command::Reset) needs: edges from
    /// the previous session must not be credited to the next player. Unlike
    /// [`Self::take`], losing a concurrent edge here is the *point*.
    pub fn discard(&self) -> u32 {
        self.count.swap(0, Ordering::Relaxed)
    }
}

impl Default for EdgeLatch {
    fn default() -> Self {
        Self::new()
    }
}

/// Clamp a count to what the protocol can carry in a `u8`.
///
/// Saturates rather than wrapping. A burst over 255 is a wiring fault or a noise
/// storm, and reporting `u8::MAX` keeps it visibly absurd instead of quietly
/// turning 256 pulses into a free game.
pub fn saturating_u8(count: u32) -> u8 {
    count.min(u8::MAX as u32) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edge_is_counted() {
        let latch = EdgeLatch::new();
        assert!(latch.record(1_000, 50));
        assert_eq!(latch.pending(), 1);
        assert_eq!(latch.take(), 1);
        assert_eq!(latch.pending(), 0);
    }

    #[test]
    fn the_very_first_edge_is_never_debounced() {
        // The clock reads 0 at this instant, which must not be mistaken for
        // "an edge was already recorded at time 0".
        let latch = EdgeLatch::new();
        assert!(latch.record(0, 50));
        assert_eq!(latch.pending(), 1);
    }

    #[test]
    fn an_edge_inside_the_debounce_window_is_rejected() {
        let latch = EdgeLatch::new();
        assert!(latch.record(1_000, 50));
        assert!(!latch.record(1_001, 50));
        assert!(!latch.record(1_049, 50));
        assert_eq!(latch.pending(), 1);
    }

    #[test]
    fn an_edge_outside_the_debounce_window_is_accepted() {
        let latch = EdgeLatch::new();
        assert!(latch.record(1_000, 50));
        assert!(latch.record(1_050, 50));
        assert_eq!(latch.pending(), 2);
    }

    #[test]
    fn bounce_does_not_extend_the_debounce_window() {
        // A rejected edge must not update `last_ms`, or a continuously bouncing
        // contact would hold the window open forever and the input would go
        // permanently deaf.
        let latch = EdgeLatch::new();
        assert!(latch.record(1_000, 50));
        for t in 1_001..1_050 {
            assert!(!latch.record(t, 50));
        }
        assert!(latch.record(1_050, 50));
        assert_eq!(latch.pending(), 2);
    }

    /// The bug this was written for, in the shape it actually appeared: one
    /// press of a real button, held for a normal length of time, arriving as two
    /// events. The second one lands while the game is between rounds and is
    /// spent instantly on whichever module lights next, so a mole gets whacked
    /// by nobody.
    #[test]
    fn a_held_press_and_its_release_bounce_are_one_event() {
        let latch = EdgeLatch::new();

        // Finger down at t=1000, bouncing for a couple of milliseconds.
        assert!(latch.record(1_000, 50));
        assert!(!latch.record(1_001, 50));
        assert!(!latch.record(1_003, 50));

        // Held for 120 ms -- unremarkable for a finger, and far longer than the
        // 50 ms window, so by the time it is let go the window has expired.
        assert!(latch.record_release(1_120, 50));

        // The contact bounces on the way open too. Every one of these is a
        // rising edge, and before `record_release` existed the first of them was
        // counted as a second press.
        assert!(!latch.record(1_120, 50));
        assert!(!latch.record(1_121, 50));
        assert!(!latch.record(1_124, 50));

        assert_eq!(
            latch.take(),
            1,
            "one press of the button must be one event, however long it is held"
        );
    }

    #[test]
    fn a_release_does_not_block_the_next_real_press() {
        let latch = EdgeLatch::new();
        latch.record(1_000, 50);
        latch.record_release(1_120, 50);
        // A second, deliberate press well after the release.
        assert!(latch.record(1_300, 50));
        assert_eq!(latch.take(), 2);
    }

    /// A press shorter than the debounce window: the release is rejected as
    /// bounce, so the window still dates from the press. The next genuine press
    /// must still be counted -- an input that goes deaf after a fast tap would
    /// be a worse bug than the one being fixed.
    #[test]
    fn a_tap_shorter_than_the_window_does_not_deafen_the_input() {
        let latch = EdgeLatch::new();
        assert!(latch.record(1_000, 50));
        assert!(!latch.record_release(1_005, 50));
        assert!(latch.record(1_200, 50));
        assert_eq!(latch.take(), 2);
    }

    /// Booting with the button held: the let-go must not be counted, and the
    /// first real press afterwards must be.
    #[test]
    fn a_release_before_any_press_is_not_an_event() {
        let latch = EdgeLatch::new();
        assert!(latch.record_release(500, 50));
        assert_eq!(latch.pending(), 0);
        assert!(latch.record(1_000, 50));
        assert_eq!(latch.take(), 1);
    }

    #[test]
    fn a_release_survives_a_wrap() {
        let latch = EdgeLatch::new();
        let before = u32::MAX - 10;
        assert!(latch.record(before, 50));
        // 60 ms later, across the wrap: accepted, and the bounce after it is not.
        assert!(latch.record_release(before.wrapping_add(60), 50));
        assert!(!latch.record(before.wrapping_add(61), 50));
        assert_eq!(latch.take(), 1);
    }

    #[test]
    fn take_leaves_an_edge_that_arrived_during_the_read() {
        // The `fetch_sub`-not-`store(0)` property promised in `take`'s docs.
        //
        // A test cannot fire a real interrupt, so it drives `take`'s two halves
        // by hand with a `record` wedged between them -- exactly where an ISR
        // would land on hardware.
        let latch = EdgeLatch::new();
        latch.record(1_000, 8);

        let observed = latch.count.load(Ordering::Relaxed);
        assert_eq!(observed, 1);

        // The interrupt lands here: after the count was read, before it was
        // cleared. This is the edge that `store(0)` would destroy.
        latch.record(1_010, 8);

        latch.count.fetch_sub(observed, Ordering::Relaxed);

        assert_eq!(
            latch.pending(),
            1,
            "an edge arriving mid-take was lost; `take` must subtract what it \
             observed, never store zero"
        );
    }

    #[test]
    fn take_one_yields_exactly_one_event_per_edge() {
        let latch = EdgeLatch::new();
        latch.record(1_000, 8);
        latch.record(1_100, 8);
        assert!(latch.take_one());
        assert_eq!(latch.pending(), 1);
        assert!(latch.take_one());
        assert!(!latch.take_one());
        assert_eq!(latch.pending(), 0);
    }

    #[test]
    fn take_settled_waits_for_the_burst_to_end() {
        let latch = EdgeLatch::new();
        latch.record(1_000, 8);
        latch.record(1_100, 8);
        // Still inside the gap: the burst may not be over.
        assert_eq!(latch.take_settled(1_200, 300), None);
        assert_eq!(latch.pending(), 2);
        // Quiet for long enough: report it as one burst of two.
        assert_eq!(latch.take_settled(1_400, 300), Some(2));
        assert_eq!(latch.pending(), 0);
    }

    #[test]
    fn take_settled_reports_nothing_when_nothing_happened() {
        let latch = EdgeLatch::new();
        assert_eq!(latch.take_settled(10_000, 300), None);
    }

    #[test]
    fn two_bursts_are_two_events() {
        let latch = EdgeLatch::new();
        latch.record(1_000, 8);
        latch.record(1_050, 8);
        assert_eq!(latch.take_settled(1_400, 300), Some(2));
        latch.record(2_000, 8);
        assert_eq!(latch.take_settled(2_400, 300), Some(1));
    }

    #[test]
    fn timestamps_wrap_without_going_deaf() {
        // ~49 days of uptime. The edge after the wrap must still be accepted,
        // and must not be treated as 4 billion milliseconds in the past.
        let latch = EdgeLatch::new();
        let before = u32::MAX - 10;
        assert!(latch.record(before, 50));
        // 60 ms later, which wraps.
        let after = before.wrapping_add(60);
        assert!(latch.record(after, 50));
        assert_eq!(latch.pending(), 2);
        // And bounce is still rejected across the wrap.
        assert!(!latch.record(after.wrapping_add(1), 50));
    }

    #[test]
    fn take_settled_survives_a_wrap() {
        let latch = EdgeLatch::new();
        let before = u32::MAX - 10;
        latch.record(before, 8);
        assert_eq!(latch.take_settled(before.wrapping_add(100), 300), None);
        assert_eq!(latch.take_settled(before.wrapping_add(400), 300), Some(1));
    }

    #[test]
    fn discard_drops_everything() {
        let latch = EdgeLatch::new();
        latch.record(1_000, 8);
        latch.record(1_100, 8);
        assert_eq!(latch.discard(), 2);
        assert_eq!(latch.pending(), 0);
        // The clock is untouched, so debounce still applies across a discard.
        assert!(!latch.record(1_101, 8));
    }

    #[test]
    fn last_ms_is_none_until_an_edge_arrives() {
        let latch = EdgeLatch::new();
        assert_eq!(latch.last_ms(), None);
        latch.record(1_234, 8);
        assert_eq!(latch.last_ms(), Some(1_234));
    }

    #[test]
    fn a_huge_burst_saturates_rather_than_wrapping() {
        assert_eq!(saturating_u8(0), 0);
        assert_eq!(saturating_u8(255), 255);
        assert_eq!(saturating_u8(256), 255);
        assert_eq!(saturating_u8(u32::MAX), 255);
    }

    #[test]
    fn a_latch_is_usable_from_a_static() {
        // The property that makes it usable from an interrupt handler: `new` is
        // `const` and every method takes `&self`.
        static LATCH: EdgeLatch = EdgeLatch::new();
        LATCH.record(1_000, 8);
        assert_eq!(LATCH.take(), 1);
    }
}
