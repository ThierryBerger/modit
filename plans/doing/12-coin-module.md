# 12 — A coin acceptor module

**Audit:** new (2026-08-28) · **Goal:** the project has a second module type
· **Size:** L · **Depends on [plan 11](11-message-protocol.md) step 1 (done)**

## Why

Everything in this project so far is a button. That is not a limitation anyone
planned — it is the shape the code fell into, and [plan
11](11-message-protocol.md) was written because of it.

A coin acceptor is the smallest thing that is genuinely *not* a button:

- It reports a **value**, not an edge. A press is a press; a coin is worth
  something.
- The value arrives as a **burst of pulses**, so the firmware has real decoding
  to do rather than forwarding an edge.
- A scenario using one needs **two kinds of module at once**, which nothing in
  the codebase could express.

It is also the module that makes an escape-game or arcade prop pay for itself,
which is the actual reason for wanting it.

## Current state

Hardware on the desk: a CH-92x-family acceptor, branded **616**. From its
datasheet: 12 V ±10 %, 50 mA idle, **350 mA momentary** (< 0.5 s), recognition
**≤ 0.6 s**. Those last two numbers set two firmware constants and one power
supply requirement; see `docs/HARDWARE.md`.

Its wires, read off the unit (2026-09-13): **`DC12V`, `COIN`, `GND`, `COUNTER`**,
plus a two-pin header labelled `SET`. `COUNTER` drives a tally counter and `SET`
is for programming; neither is used.

**Bring-up moves to an Arduino Uno** (2026-09-13) -- a Kuman Uno, an Uno R3
clone. The ESP32 firmware stays written and parked. The reason is the computer,
not the circuit: anything that talks to the brain over USB puts the laptop on the
same ground as the 12 V supply, and a wiring mistake on the bench could then
reach its USB port. The Uno runs from the acceptor's own 12 V supply through its
barrel jack, with USB unplugged, so the laptop is never on that circuit. See
[why an Uno first](#why-an-arduino-uno-first).

The Uno has no radio. **It does not talk to the brain yet**; that is work in
progress and deliberately not designed here.

## Target state

First, on the Uno: **an acceptor that counts coins correctly on its own**, one
LED blink per pulse.

Then, the module proper: a `modit-coin-<id>` board that reports `Event::Coin { channel, pulses }`, plus
a scenario that takes money.

**The firmware reports pulses, not currency.** What a pulse is worth depends on
how the acceptor was programmed and what a venue charges, and that is policy: it
belongs in the scenario, which is the part you are meant to edit, not in the
firmware you flash once.

## Steps

### 1. Protocol (done, no hardware)

- [x] `Role::Coin` and `Event::Coin { channel, pulses: u8 }` in `shared::proto`.
- [x] `uuids::COMMAND_WRITE` / `uuids::EVENT_NOTIFY` — plan 11's "one service,
      two characteristics, forever".
- [x] A golden-bytes test proving the append did not move any existing variant,
      which is *why* `PROTOCOL_VERSION` stayed at 1.

### 2. Make capability discovery real (done, no hardware)

- [x] Populate `Modules::descriptors`. It was constructed as an empty `HashMap`
      and never filled, so `descriptor()` always returned `None` and every
      `require()` call silently skipped its check. See the notes below.
- [x] `runtime::require_role`, so a scenario can insist on a `Role::Coin`.
- [x] Hold back non-`Hello` events arriving during the sweep and replay them in
      order, so a press during connect is not eaten.

### 3. Simulator and scenario (done, no hardware)

- [x] `SimLink::new` takes a `Descriptor` per module instead of one channel count
      for all of them.
- [x] `SimHandle::insert_coin`, and `$` / `$3` at the `--simulate` prompt.
- [x] `Modules::next_coin`, a filter over `next_event` like `next_press`.
- [x] The `arcade` scenario: attract mode, a coin buys credits, credits are
      played out.
- [x] Tests, including that a 0-pulse coin buys nothing and that a bench without
      a coin acceptor is refused rather than hung.

### 4. Firmware

#### 4a. Arduino Uno (**current**, not started)

Standalone first: nothing here talks to the brain. Wiring, nets and checks are
in [module-coin](../../docs/hardware/module-coin.md), Arduino Uno tabs.

Before any code, with a multimeter and the 12 V supply (USB unplugged):

- [x] Identify the wires: `DC12V`, `COIN`, `GND`, `COUNTER`, and a `SET`
      header.
- [ ] Confirm the labels with a meter against `GND`: `DC12V` at 12 V, `COIN`
      floating. `COUNTER` and `SET` stay unconnected.
- [ ] Set the NO/NC switch to NO, if the unit has one.
- [ ] The acceptor alone, on 12 V, accepts a coin it was taught.
- [ ] `D2` reads ~5 V at rest with the acceptor wired and powered.

Toolchain and shared code:

- [ ] Install the AVR toolchain: `avr-gcc` and `avrdude` (Homebrew,
      `osx-cross/avr`), `ravedude` (`cargo install ravedude`). Confirm the Uno's
      CH340 shows up as `/dev/cu.usbserial-*`.
- [ ] `crates/module-coin-uno`: a separate workspace like the other firmware,
      `avr-hal` for the ATmega328P, nightly pinned in `rust-toolchain.toml`,
      `ravedude` as the runner.
- [ ] Make `shared::input` build for `avr-atmega328p`. AVR has no atomics at
      all, so `AtomicU32` becomes `portable-atomic`'s, with the host tests
      unchanged. Check whether `shared` as a whole builds there (`postcard`,
      `serde`); if it does not, feature-gate `proto`, which step 4a does not
      need.
- [ ] Move `PULSE_DEBOUNCE_MS` and `BURST_GAP_MS` from `module-coin` into
      `shared`, so the two firmwares cannot disagree about what a burst is.
- [ ] `just flash-coin-uno`.

Firmware:

- [ ] A millisecond clock from `TC0`, for `EdgeLatch`'s timestamps.
- [ ] Falling-edge `INT0` on `D2`, **internal pull-up off** -- enabled, it sits on
      the pin's side of `R2` and lifts a low to ~1.1 V.
- [ ] Self-test: three slow blinks of `D13`. If `D2` reads low at rest, blink
      fast forever and count nothing.
- [ ] Otherwise blink `D13` *n* times for each *n*-pulse burst.

On the hardware (USB unplugged, 12 V supply in):

- [ ] Post a coin: one group of blinks, not one blink per pulse spread over
      seconds.
- [ ] A coin taught as *n* pulses blinks *n* times, ten coins in a row.
- [ ] The Uno does not reset when the solenoid fires (a reset shows as the
      self-test's three slow blinks).
- [ ] Extend `wiring_tables_match_firmware` to the Uno: `D`*n* rows in the net
      table against the `pins.d`*n* the firmware takes. Until then that table
      is unchecked, and the page says so.

Not in this step: getting coins from the Uno to the brain. It is not supported
yet (work in progress).

#### 4b. ESP32 (written, **needs the hardware**, parked)

- [x] `crates/module-coin`, speaking `shared::proto` over the two new
      characteristics. Compiles and links; `just build-firmware` covers it.
- [x] Falling-edge interrupt with an 8 ms debounce; bursts closed by 300 ms of
      quiet.
- [x] Boot self-test: report the pulse line's idle level.
- [ ] **Meter every acceptor wire against `GND` before wiring anything.**
- [ ] **Flash it and post a coin.** Nothing below this line has been run against
      real hardware.
- [ ] Confirm the pulse line idles high on the internal pull-up alone.
- [ ] Confirm a coin taught as *n* pulses arrives as one `Coin { pulses: n }`,
      not as *n* separate events — this is what `BURST_GAP_MS` buys.
- [ ] Confirm the ESP32 does not brown out when the coil fires (the 350 mA spike).

### 5. Coin over BLE (**not started**)

This step is about the ESP32 firmware. The Uno from step 4a has no radio and is
not covered by it.

The blocker, and the honest limitation of everything above: `arcade` runs under
`--simulate` only.

`link/ble.rs` is typed to `ButtonLed`/`ButtonDetails` and still translates the
*old* wire format — `&[0]`/`&[1]` out, `b"Notification"` in. A coin's pulse count
cannot be expressed in that format at all, so there is nowhere to put a coin
module. `brain` says so and refuses rather than scanning for a board it could not
drive.

This is exactly [plan 11](11-message-protocol.md) steps 2–3, and doing it needs
the button firmware moved onto `shared::proto` as well:

- [ ] Add `COMMAND`/`EVENT` to `module-button` alongside its existing
      characteristics (plan 11 step 2 — "both work at once", so nothing is lost
      if it is wrong).
- [ ] Make the BLE link speak `proto::encode`/`decode` over those two
      characteristics, for any module, with no per-role plumbing. It should get
      *smaller*: the role only affects the advertised name.
- [ ] Replace `Bench::button_modules` with something role-agnostic and delete the
      refusal in `main`.
- [ ] Plan 11 step 4: delete the old characteristics, `Notifier`, `Writable`, and
      `firmware_uuids_match_shared`.

## Done when

- [ ] The Uno counts a coin taught as three pulses as three blinks, repeatably,
      with the laptop unplugged.
- [ ] `just brain --scenario arcade` takes a real coin and starts a real game.
- [ ] A coin taught as three pulses gives three credits, repeatably.
- [ ] `grep -rn "cannot drive one yet" crates/` finds nothing.

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
fine for a button. It is not fine here. A pulse can be ~30 ms, and one BLE
iteration overrunning that does not lose an event, it changes the coin's
*value* — a two-credit coin silently becomes a one-credit coin. That failure
would be intermittent, load-dependent, and nearly impossible to attribute.

### Why the burst gap is 300 ms

Not a guess, and worth not re-guessing: pulses within one burst are at most
~100 ms apart even on the slowest pulse-width setting, and the datasheet's 0.6 s
recognition time means two coins cannot produce bursts closer together than that.
300 ms sits between the two with room on both sides. Changing the acceptor's
pulse-speed switch invalidates it.

### Why there is no isolation

The first draft of this module put an optocoupler on the COIN line. It did not
earn its keep: the COIN output is **open-collector**, so whatever pulls it up
decides its idle voltage. The board's pull-up already does that, and 12 V never
reaches the wire. The opto was protecting against a hazard the interface had
already removed. The series resistor stays -- not for function, but so that
identifying the wrong wire on an unlabelled clone costs little.

What the optos did buy is a floating ground, and dropping them means the ESP32's
`GND` must be bonded to the 12 V supply's. The cost is that the 350 mA solenoid
spike now shares the ESP32's ground reference. On a bench that is fine. If a
coin is ever miscounted at a venue with a long ground run, this note is the first
place to look — and the fix is a ground bond at the supply, not a rebuild.

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

### Deliberately not decided here

- **Whether `Event::Coin` should be a general `Counted { channel, count }`**,
  which would also fit a turnstile or a pulse water meter. `Coin` was chosen for
  the better log lines, on the same reasoning plan 11 used for keeping `Role`:
  start specific, relax later. Relaxing it is a rename.
- **Crediting across a disconnect.** Pulses counted but unreported are discarded
  on `Reset`, and lost outright if the board reboots. Making that safe means the
  module holding credit in NVS and the brain acknowledging it, which wants the
  sequence numbers plan 11 also deferred. A dropped coin is currently a lost
  coin; for a game that is annoying, and for a real cash box it would not be
  acceptable.
