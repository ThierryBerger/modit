# 14 — Catch every input, not just the slow ones

**Audit:** new (2026-09-03) · **Goal:** an input event cannot be silently dropped
· **Size:** M · **Prerequisite for [plan 15](../todo/15-impact-targets.md)**

## Why

`module-button` samples its pin once per pass of the BLE work loop:

```rust
let button_is_high = button.is_high();
let rising_edge = button_is_high && !button_was_high;
```

That loop's period is **unbounded**. `do_work_with_notification` performs HCI
I/O whose duration depends on what the radio is doing, so the sampling rate is
whatever is left over after BLE. A finger press lasts 50–200 ms and survives any
plausible overrun, which is why this has never been a problem and why the code is
written this way on purpose -- `module-coin`'s module docs say so out loud.

It stops being true the moment an input is fast. A tennis ball in contact with a
rigid plate is **4–6 ms**. Detection then becomes a race between the impact
window and the radio, and the failure mode is the worst one available: the hit is
*silently dropped*, and the player reads it as having missed. There is no error,
no log line, nothing to debug.

`module-coin` already solved this, correctly, for the same reason -- a missed
pulse does not lose an event, it changes a two-credit coin into a one-credit
coin. **This plan generalises that solution and moves `module-button` onto it**,
so that a fast input is a supported thing rather than a thing one firmware
happens to handle.

The prize is not only the ball game. It is that the answer to "can a modit module
detect X" stops depending on which firmware you copied from.

## Current state

**`module-coin` -- correct, and the reference for this plan:**

- `crates/module-coin/src/main.rs:180` -- `#[handler] fn coin_pulse()`, an ISR
  that reads the clock, debounces, bumps an `AtomicU32`, and clears the
  interrupt. Nothing else.
- `crates/module-coin/src/main.rs:154` -- `COIN_PIN:
  Mutex<RefCell<Option<Input<'static>>>>`, so the ISR can reach the pin.
- `crates/module-coin/src/main.rs:203` -- `finished_burst()`, which decides a
  burst has ended after `BURST_GAP_MS` of quiet.
- Timestamps are `AtomicU32` milliseconds compared with `wrapping_sub`, because
  **the Xtensa ESP32 core has no 64-bit atomics**. This is not a detail to
  rediscover later.
- The clear-with-`fetch_sub`-not-`store(0)` trick: a pulse arriving between
  reading the count and clearing it is carried into the next burst instead of
  being lost.

**`module-button` -- the polling version described above,
`crates/module-button/src/main.rs:229`, with `DEBOUNCE_MS = 50` applied in the
main loop.**

Neither firmware's debounce logic is tested. It cannot be: it is entangled with
`esp_hal::Input` and only compiles for `xtensa-esp32-none-elf`.

## Target state

Split the problem where it naturally splits.

### 1. The logic, pure and host-tested, in `shared`

A latch holding only integers, with **time as an argument rather than a
dependency**:

```rust
pub struct EdgeLatch {
    count:   AtomicU32,
    last_ms: AtomicU32,
}

impl EdgeLatch {
    /// Record an edge. Returns false if it was rejected as bounce.
    pub fn record(&self, now_ms: u32, debounce_ms: u32) -> bool;
    /// Take everything committed so far.
    pub fn take(&self) -> u32;
    /// Take a burst, but only once `gap_ms` of quiet says it has ended.
    pub fn take_settled(&self, now_ms: u32, gap_ms: u32) -> Option<u32>;
    /// Throw away anything uncounted -- what `Command::Reset` needs.
    pub fn discard(&self) -> u32;
}
```

Atomics only, `u32` only, no allocation, no `esp_hal`, no `async`. It therefore
compiles for the host, and **the debounce and burst rules become `cargo test`
material** rather than something verified by posting coins into a physical slot.

This is the same move [plan 13](../todo/13-brainless-token-mesh.md) step 2 makes for the
mesh state machine, for the same reason.

### 2. The wiring, thin, in each firmware

Each firmware keeps its own `#[handler]`, because the handler must clear *its*
pin's interrupt and the pin type is `esp_hal`'s. The handler shrinks to two
lines: read the clock, call `record`. Everything interesting is in the tested
half.

### What this is not

- **Not a protocol change.** `module-button` keeps sending `b"Notification"`;
  moving it onto `shared::proto` is [plan 11](11-message-protocol.md)
  step 2 and stays there. Changing *detection* and *encoding* in one commit would
  make a hardware regression impossible to attribute.
- **Not a behaviour change for `module-coin`.** Its constants and semantics are
  tuned against a real acceptor. The extraction must be provably a no-op for it,
  which is exactly why it is the migration that goes first.

## Steps

### 1. Extract the latch, with tests (no hardware) — **done**

- [x] Add `shared::input` with `EdgeLatch` as above.
- [x] Host tests, and these are the point of the whole step:
  - [x] An edge inside the debounce window is rejected; outside it, accepted.
  - [x] The first edge ever (`last_ms == 0`) is accepted.
  - [x] `take_settled` returns `None` mid-burst and `Some(n)` after the gap.
  - [x] **An edge arriving between the read and the clear is not lost** -- the
        `fetch_sub` property, written as a test so nobody "simplifies" it back
        into `store(0)`.
  - [x] Timestamps wrap at `u32::MAX` without hanging or mis-counting more than
        one boundary.
  - [x] A burst larger than `u8::MAX` saturates rather than wrapping.
- [x] `shared` gains no new dependencies. Pure `core::sync::atomic`.

### 2. Move `module-coin` onto it (proves the extraction) — **done**

- [x] Replace `PULSES` / `LAST_PULSE_MS` / `finished_burst` with an `EdgeLatch`.
- [x] `coin_pulse` becomes: `LATCH.record(now_ms(), PULSE_DEBOUNCE_MS)`, then
      clear the interrupt.
- [x] `Command::Reset` calls `discard()`.
- [x] **Diff review against the old behaviour, constant by constant.** If this
      step changes anything observable, the extraction is wrong.

### 3. Move `module-button` onto it (the actual fix) — **code done, needs a board**

- [x] `Io::set_interrupt_handler` and `button.listen(GpioEvent::RisingEdge)` --
      rising, because the pin has a pull-down and idles low.
- [x] Delete `button_was_high`, the polled sample, and the main-loop debounce.
- [x] Each pass of the loop drains the latch instead of sampling the pin.
- [x] Presses arriving while no client is subscribed are **discarded, not
      queued** -- matching today's behaviour, and stated as a comment so it reads
      as a decision rather than an oversight.
- [x] Keep `DEBOUNCE_MS = 50` for now. Retuning belongs to plan 15, against a
      measurement.

### 3b. Debounce the release, not just the press — **code done, needs a board**

Found by playing the game on three boards (2026-09-07): a light appearing and
vanishing instantly, with nobody touching it, then the next round starting
normally after its usual pause.

`record` measures the debounce window from the last **accepted** edge. A press
held for longer than the window -- which is every press a finger makes -- leaves
the window expired by the time the button is let go, and a switch bounces on
release exactly as it does on press. Each of those closures is another rising
edge, so **one press produced two events**, the second arriving 50-200 ms behind
the first. That second event landed while the game was between rounds and was
spent on whatever lit next.

Step 4 below already had the check that would have caught this -- "confirm that
holding the button still produces exactly one event" -- and it was never run.

- [x] `EdgeLatch::record_release`: counts nothing, restarts the debounce window,
      so bounce on the release falls inside it.
- [x] Host tests for the real shape of it: a press held 120 ms plus its release
      bounce is **one** event; a tap shorter than the window does not deafen the
      input; a release before any press is not an event; both survive a `u32`
      wrap.
- [x] `module-button` arms `GpioEvent::AnyEdge` and the handler reads the pin
      level to tell press from release.
- [x] **Not applied to `module-coin`.** Its pulse line is a transistor output
      rather than a contact, and step 2 promised that moving it onto the latch
      changes nothing observable. Decided against a real acceptor (2026-09-13):
      `module-coin-uno` arms the falling edge only, calls `record` and never
      `record_release`, and counts every coin exactly. There is no release
      bounce to suppress.
- [ ] Confirm on hardware, with the check from step 4.

Belt and braces on the host side, in `brain`: `whack_a_mole` now drops presses
that arrived **before** the target lit (`Modules::drop_pending_presses`). A press
before the light is not a hit on it, whatever the firmware does.

### 4. Verify on hardware — **blocked: needs two boards**

- [ ] Coin module: post coins, confirm identical burst counts to before. The
      latch itself is confirmed on real coins by `module-coin-uno` (same
      `EdgeLatch`, same `shared::coin` timing); what is left is the ESP32 build.
- [ ] Button module: confirm presses still register, and that holding the button
      still produces exactly one event. **This is the one that matters** -- see
      step 3b, which is the bug it would have caught. Hold the button for a full
      second: the serial log must print `button pressed, notifying` exactly once,
      on the way down and not on the way up.
- [ ] **The test that motivates all of this:** tap the button pin briefly with a
      wire while the radio is busy, and confirm nothing is dropped. A dupont wire
      brushed against 3V3 is a decent stand-in for a fast edge.

### 5. Write down what it bought

- [ ] Record the shortest edge reliably caught, in the table below. Plan 15
      needs that number to choose a sensor front-end.

## Done when

- [x] `cargo test` covers debounce, burst settling, the lost-edge race and
      wrap-around, with no hardware and no `esp_hal`.
- [x] Neither firmware polls a GPIO in the BLE loop.
- [ ] `module-coin` behaves identically to before, confirmed with real coins.
- [x] `grep -n "is_high()" crates/module-button/src/main.rs` finds only the
      self-test.
- [ ] The shortest reliably-detected edge is measured and recorded.

## Notes

### Why the logic goes in `shared` and not a new firmware crate

The tempting alternative is `crates/module-common`, a `no_std` crate depending on
`esp-hal` that both firmwares use. It would also house `custom_halt`, `MODIT_ID`
and the advertising boilerplate, which are genuinely duplicated.

It was rejected **for this plan** because anything depending on `esp-hal` only
builds for `xtensa-esp32-none-elf`, which means the debounce logic would stay
untestable -- and untestable debounce logic is the thing this plan exists to fix.
A `module-common` for the *wiring* is still a good idea later; it is just not
where the value is today.

### Why `u32` milliseconds and not a real instant

The Xtensa ESP32 has no 64-bit atomics, so an `AtomicU64` timestamp does not
exist to be shared with an ISR. `u32` milliseconds wraps after ~49 days; every
comparison uses `wrapping_sub`, so a wrap costs at most one mis-timed boundary
instead of a hang. `module-coin` already learned this; the extraction must not
quietly un-learn it.

### What landed, 2026-09-03

Steps 1-3 are written and the full `just ci` gate is green: host tests, clippy
with `-D warnings`, and a **release build of both firmwares** (which is the step
that link-checks the panic handler). Step 4 is the only thing left and it needs
boards.

`shared::input::EdgeLatch` gained one method that was not in the sketch above:
**`take_one`**. It is needed because `do_work_with_notification` carries exactly
one notification per pass, so a button draining with `take()` would swallow every
press but the first -- reintroducing, in miniature, the dropped input this plan
exists to remove. `take` and `take_settled` remain for the coin, where several
edges together name one thing.

**One deliberate behaviour change**, and it is a fix rather than a no-op:

`module-coin`'s old ISR accepted its first pulse because `last_ms` was 0 and
`now.wrapping_sub(0)` exceeds any debounce window at any real uptime. That is
true but accidental -- it relies on the board having been up for at least 8 ms.
`EdgeLatch` tracks "has an edge ever been seen" explicitly instead, so a first
edge is accepted unconditionally. Same behaviour in every reachable case, one
fewer assumption. The coin module is otherwise byte-for-byte equivalent in
behaviour; its constants were not touched.

`module-button` also now logs discarded presses (`"N press(es) with no client
subscribed"`), where before it logged one line per press. Cosmetic, but it will
look different on the serial monitor during step 4 -- do not read it as a
regression.

### Deliberately not decided here

- **Whether an ISR should timestamp *every* edge into a ring buffer** rather than
  collapsing to a count. That would allow measuring impact duration, which
  plan 15 might want. It costs memory and complexity, so wait until there is a
  measurement asking for it.
- **Retuning `DEBOUNCE_MS`.** 50 ms is right for a finger and probably wrong for
  a ringing plate. Plan 15 owns that, with numbers.

### Measurements

| What | Expected | Measured |
| ---- | -------- | -------- |
| Shortest edge reliably caught, radio idle | < 1 ms | |
| Shortest edge reliably caught, radio busy | < 1 ms | |
| Worst observed BLE loop iteration (the old sampling period) | ? | |
