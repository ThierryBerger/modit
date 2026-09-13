# 12 — A coin acceptor module: protocol, simulator, and the Uno bring-up

**Audit:** new (2026-08-28) · **Done:** 2026-09-13 · **Goal:** the project has a
second module type · **Size:** L · **Depends on [plan 11](../doing/11-message-protocol.md)
step 1 (done)**

This is the finished half of plan 12. What is left -- the ESP32 on a real
acceptor, and coins over BLE -- is
[`doing/12-coin-module-esp32.md`](../doing/12-coin-module-esp32.md).

## Why

Everything in this project so far is a button. That is not a limitation anyone
planned — it is the shape the code fell into, and [plan
11](../doing/11-message-protocol.md) was written because of it.

A coin acceptor is the smallest thing that is genuinely *not* a button:

- It reports a **value**, not an edge. A press is a press; a coin is worth
  something.
- The value arrives as a **burst of pulses**, so the firmware has real decoding
  to do rather than forwarding an edge.
- A scenario using one needs **two kinds of module at once**, which nothing in
  the codebase could express.

It is also the module that makes an escape-game or arcade prop pay for itself,
which is the actual reason for wanting it.

## What was built

- **Host side:** `Role::Coin` and `Event::Coin`, real capability discovery, a
  simulator that posts coins, and the `arcade` scenario.
- **An Arduino Uno that counts coins on its own** (`crates/module-coin-uno`): one
  LED blink per pulse, one group per coin, running from the acceptor's 12 V
  supply with the laptop unplugged. It has no radio and does not talk to the
  brain.

**The firmware reports pulses, not currency.** What a pulse is worth depends on
how the acceptor was programmed and what a venue charges, and that is policy: it
belongs in the scenario, which is the part you are meant to edit, not in the
firmware you flash once.

## Confirmed on the bench (2026-09-13)

The acceptor is a CH-92x-family unit branded **616**: 12 V ±10 %, 50 mA idle,
**350 mA momentary** (< 0.5 s), recognition **≤ 0.6 s**. Driven by a Kuman Uno
(an Uno R3 clone) wired as [module-coin](../../docs/hardware/module-coin.md)'s
Arduino Uno tabs describe, USB unplugged, one 12 V supply for both.

What that bench established, and which the rest of the project can now rely on:

| Finding | How it is known |
| ------- | --------------- |
| The wires are what they are labelled: **`DC12V`, `COIN`, `GND`, `COUNTER`**, plus a two-pin `SET` header. `COUNTER` and `SET` are unused. | Read off the unit, then confirmed by function: the acceptor runs, and `COIN` carries the pulses. Not metered. |
| The unit **has an NO/NC switch**; on **NO**, `COIN` idles high on an external pull-up and each pulse pulls it low. | The self-test reads `D2` high at rest on 12 V, and falling edges count. |
| `COIN` is an **open-collector output**: pulled up to the board's own 5 V, 12 V never appears on it. | The circuit works with nothing but `R1` to `5V` defining the high level, and the pin survived. |
| **8 ms debounce and a 300 ms burst gap** split coins correctly: each coin is one group of exactly its taught pulse count, repeatably, coin after coin. | Several coins in a row, each blinking its pulse count once. |
| **One 12 V supply for acceptor and Uno is enough.** The solenoid's 350 mA spike does not reset the Uno. | No self-test blinks reappeared when coins were accepted. |
| `R2` (10 k in series with `D2`) and the internal pull-up **off** do not disturb counting. | Same counts, same bench. |

The first two rows are facts about the acceptor, not about the Uno, so they hold
for the ESP32 wiring too.

## Steps

### 1. Protocol (no hardware)

- [x] `Role::Coin` and `Event::Coin { channel, pulses: u8 }` in `shared::proto`.
- [x] `uuids::COMMAND_WRITE` / `uuids::EVENT_NOTIFY` — plan 11's "one service,
      two characteristics, forever".
- [x] A golden-bytes test proving the append did not move any existing variant,
      which is *why* `PROTOCOL_VERSION` stayed at 1.

### 2. Make capability discovery real (no hardware)

- [x] Populate `Modules::descriptors`. It was constructed as an empty `HashMap`
      and never filled, so `descriptor()` always returned `None` and every
      `require()` call silently skipped its check. See the notes below.
- [x] `runtime::require_role`, so a scenario can insist on a `Role::Coin`.
- [x] Hold back non-`Hello` events arriving during the sweep and replay them in
      order, so a press during connect is not eaten.

### 3. Simulator and scenario (no hardware)

- [x] `SimLink::new` takes a `Descriptor` per module instead of one channel count
      for all of them.
- [x] `SimHandle::insert_coin`, and `$` / `$3` at the `--simulate` prompt.
- [x] `Modules::next_coin`, a filter over `next_event` like `next_press`.
- [x] The `arcade` scenario: attract mode, a coin buys credits, credits are
      played out.
- [x] Tests, including that a 0-pulse coin buys nothing and that a bench without
      a coin acceptor is refused rather than hung.

### 4a. Arduino Uno, standalone

The acceptor:

- [x] Identify the wires: `DC12V`, `COIN`, `GND`, `COUNTER`, and a `SET`
      header.
- [x] Confirm the labels: `DC12V` powers the acceptor, `COIN` is the pulse
      output. Confirmed by function rather than with a meter.
- [x] Set the NO/NC switch to NO.
- [x] The acceptor, on 12 V, accepts the coins it was taught.
- [x] `D2` reads high at rest with the acceptor wired and powered, as read by the
      firmware's self-test.

Toolchain and shared code:

- [x] Install the AVR toolchain: `avr-gcc`, `avrdude`, `ravedude` ≥ 0.2
      (`just setup-avr`, Debian/Ubuntu). The Kuman Uno has an ATmega16U2, not a
      CH340: it enumerates as `2341:0043` on `/dev/ttyACM0`. `Ravedude.toml`
      lists the CH340's id as well, for clones that do carry one.
- [x] `crates/module-coin-uno`: a separate workspace like the other firmware,
      `avr-hal` for the ATmega328P, nightly pinned in `rust-toolchain.toml`,
      `ravedude` as the runner. `encoding_rs` (a build dependency of
      `avr-device`) is held at 0.8.35 in the lockfile; newer needs a newer rustc.
- [x] Make `shared::input` build for `avr-none`: `AtomicU32` is
      `portable-atomic`'s, host tests unchanged. `shared` as a whole builds
      there, `postcard` and `serde` included, so `proto` is not feature-gated.
- [x] Move `PULSE_DEBOUNCE_MS` and `BURST_GAP_MS` from `module-coin` into
      `shared::coin`, so the two firmwares cannot disagree about what a burst is.
- [x] `just flash-coin-uno`, plus a CI job building the crate.

Firmware:

- [x] A millisecond clock from `TC0`, for `EdgeLatch`'s timestamps.
- [x] Falling-edge `INT0` on `D2`, **internal pull-up off** -- enabled, it sits on
      the pin's side of `R2` and lifts a low to ~1.1 V.
- [x] Self-test: three slow blinks of `D13`. If `D2` reads low at rest, blink
      fast forever and count nothing.
- [x] Otherwise blink `D13` *n* times for each *n*-pulse burst. Each burst is
      also printed on the serial console, for when USB is the power source.
- [x] Extend `wiring_tables_match_firmware` to the Uno: `D`*n* rows in the net
      table against the `pins.d`*n* the firmware takes.

On the hardware (USB unplugged, 12 V supply in):

- [x] Boot: three slow blinks, no fast-blink fault.
- [x] Post a coin: one group of blinks, not one blink per pulse spread over
      seconds.
- [x] A coin taught as *n* pulses blinks *n* times, coin after coin.
- [x] The Uno does not reset when the solenoid fires.

## Done when

- [x] The Uno counts a coin taught as *n* pulses as *n* blinks, repeatably, with
      the laptop unplugged.

## Notes

### `Modules::descriptors` was never populated

Found while writing `arcade`, which cannot sort a coin slot from a button
without it. `runtime::run_once` built `Modules { descriptors: HashMap::new(), .. }`
and nothing ever wrote to it: the `Hello` events flowed through the event channel
to scenarios as ordinary events and were dropped on the floor by every scenario's
wait loop.

The consequence is worth recording, because it was invisible: **every `require()`
call in every scenario has been a no-op since it was written.** It took the
`debug!("no descriptor for {id}, skipping the capability check")` branch every
time. Plan 11's headline win — "a scenario wanting output channel 2 from a module
reporting `outputs: 1` should fail loudly at bind time" — was not actually
happening.

Nothing had noticed because the only modules were buttons and every scenario
asked for `(1, 1)`, which is what they are.

### Why the pulse line is an interrupt

`module-button` samples its pin once per pass of the BLE work loop, and that is
fine for a button. It is not fine here. A pulse can be ~30 ms, and one loop
iteration overrunning that does not lose an event, it changes the coin's
*value* — a two-credit coin silently becomes a one-credit coin. That failure
would be intermittent, load-dependent, and nearly impossible to attribute.

The Uno relies on it too: its main loop sits in blocking `delay_ms` calls for
the whole length of a blink group, and only the interrupt keeps counting while it
does.

### Why the burst gap is 300 ms

Not a guess, and worth not re-guessing: pulses within one burst are at most
~100 ms apart even on the slowest pulse-width setting, and the datasheet's 0.6 s
recognition time means two coins cannot produce bursts closer together than that.
300 ms sits between the two with room on both sides, and `shared::coin` asserts
that ordering at compile time. On the bench it separates coins correctly at the
616's current pulse-speed setting. Moving that switch is the one thing that
invalidates it.

### Why there is no isolation

The first draft of this module put an optocoupler on the COIN line. It did not
earn its keep: the COIN output is **open-collector**, so whatever pulls it up
decides its idle voltage. The board's pull-up already does that, and 12 V never
reaches the wire -- confirmed on the Uno bench. The opto was protecting against a
hazard the interface had already removed. The series resistor stays -- not for
function, but so that identifying the wrong wire on an unlabelled clone costs
little.

What the optos did buy is a floating ground, and dropping them means the board's
`GND` must be bonded to the 12 V supply's. The cost is that the 350 mA solenoid
spike now shares the board's ground reference. On the bench, with the acceptor's
`GND` on its own return wire to the supply, that costs nothing: counts are exact
and the Uno does not reset. If a coin is ever miscounted at a venue with a long
ground run, this note is the first place to look — and the fix is a ground bond
at the supply, not a rebuild.

### Why an Arduino Uno first

The first suggestion was to wire the Uno to the brain over USB. That was dropped:
USB joins the laptop's ground to the 12 V supply's, which is fine when the wiring
is right and puts the laptop's USB port one mistake away from 12 V when it is
not. On a bench with unlabelled clone wires, the mistake is the likely case.

Running the Uno from the acceptor's supply removes the laptop from the circuit.
The USB cable is only for flashing, and never plugged in with the 12 V supply.

What the Uno does **not** change, since it is tempting to think 5 V is "closer"
to 12 V: `COIN` is open-collector either way, and the pull-up just moves from
3.3 V to 5 V.

What it does buy: 5 V pins with a published tolerance for a small clamp current,
a circuit that matches the widely copied CH-926 Arduino tutorial, and a board
where a mistake costs a few euros and nothing else.

The one change from that tutorial is `R2`, 10 k in series with `D2`, with the
10 k pull-up on the acceptor's side of it. There, `R2` carries only leakage in
normal operation, so it does not affect the low level, and it holds a
mis-identified 12 V wire to ~0.65 mA into the pin.

The ESP32 page has the weaker version of that insurance: its `R1` is 1 k, which
lets about 8 mA into the pin on the same mistake. The page said "about a
milliamp" until this was worked out.

### The Uno's firmware leans on `shared`, not the reverse

`EdgeLatch` was written for the ESP32 and never touched a clock or a HAL, so it
ported unchanged: the only AVR-specific change in `shared` is where `AtomicU32`
comes from. That makes the bench result above evidence about the shared counting
rules themselves, not only about this one firmware.
