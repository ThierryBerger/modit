# 15 — Targets you hit with a ball

**Audit:** new (2026-09-03) · **Goal:** a module survives being hit, and notices
· **Size:** L · **Needs [plan 14](../doing/14-edge-latched-inputs.md) first**

## Why

The games so far are pressed with a finger. The one that wants building is
played with a racket: targets on a wall or on stands, and you hit the lit one
with a tennis ball. It is the same game loop -- one target is lit, hit it, the
next lights -- so [plan 13](13-brainless-token-mesh.md)'s token passing carries
it unchanged.

What does not carry unchanged is the hardware. A push button is the wrong
component by roughly two orders of magnitude, and the reason is worth writing
down once so it is not re-argued.

### The arithmetic

A tennis ball is ~58 g. Contact with a rigid surface lasts **4–6 ms**, with a
coefficient of restitution around 0.75, so the ball leaves at roughly 0.75× the
speed it arrived and the momentum change is ~1.75× the incoming momentum.

| Incoming | Δv | Impulse | Mean force over 5 ms | Rough peak |
| -------- | -- | ------- | -------------------- | ---------- |
| 10 m/s (36 km/h, a lob) | 17.5 m/s | 1.0 N·s | ~200 N | ~320 N (33 kgf) |
| 20 m/s (72 km/h, a rally) | 35 m/s | 2.0 N·s | ~405 N | ~640 N (65 kgf) |
| 40 m/s (144 km/h, a serve) | 70 m/s | 4.1 N·s | ~1000 N | ~1600 N (160 kgf) |

A tactile switch actuates at 1–3 N. A good arcade microswitch tolerates perhaps
20–50 N of static load. **The gentlest case is an order of magnitude past that**,
delivered as a shock rather than a push. The button does not wear out; it breaks.

There is a second problem that no amount of ruggedness fixes: the ball is 67 mm
of deformable fuzzy rubber, and it will squash *around* a small plunger rather
than depress it. Even an indestructible button would trigger unreliably.

So the impact must be absorbed by a plate and detected by something with no
moving parts.

## Current state

- `docs/HARDWARE.md` -- one momentary push button on GPIO33 (pull-down, so it
  idles low), one LED on GPIO26 through a 220 Ω–1 kΩ resistor, and the advice
  "a breadboard and jumper wires for the first one".
- `crates/module-button/src/main.rs` -- polls the pin in the BLE loop
  (**plan 14 fixes this, and must land first**), `DEBOUNCE_MS = 50`.
- `shared::proto::Event` already has `Measurement { channel, value }`, which is
  the natural carrier for "how hard", and `Pressed { channel }` for "at all".
- `crates/module-coin` is the in-tree precedent for a module whose input needs
  real analog thinking and a protective front-end.

## Target state

A **target module**: a rigid plate, an impact sensor behind it, a light bright
enough to see from across the room, and an ESP32 that survives the shock.

### Choosing the sensor

| Approach | Response | Cost | Verdict |
| -------- | -------- | ---- | ------- |
| **Piezo disc** bonded behind a rigid plate | Voltage spike, sub-ms, amplitude ∝ force | ~€0.50 | **Start here.** How electronic drum pads work. No moving parts, effectively unlimited life. |
| **Accelerometer** (LIS3DH) bonded to the plate | Built-in click engine, INT pin → GPIO | ~€3 | **Best end state.** Threshold becomes software, tunable over I²C with no soldering iron. |
| **Break-beam** (38 kHz modulated IR) | Microseconds, no contact at all | ~€2 | Only if the game becomes "hit *through* a hoop". Needs modulation to survive daylight. |
| **Hinged flap** → microswitch behind it | Flap eats the impact; switch sees a slow event | ~€2 | The escape-room answer. Forgiving, but has moving parts and rebound. |
| FSR / load cell | — | — | **No.** FSRs degrade past their range; HX711 samples at 10–80 Hz. |

The piezo and the accelerometer are the same design with the analog work moved
across the hardware/software line. Prototype with the piezo because it is cheap
enough to destroy; consider the LIS3DH once the thresholds are known, because by
then the tuning is worth having in software.

### The piezo front end

A piezo disc is a **generator**, not a sensor you power: deforming the ceramic
produces the voltage itself, tens of volts, with no supply rail involved and no
current limit. Straight into a GPIO it destroys the pin, exactly as
[`docs/HARDWARE.md`](../../docs/HARDWARE.md) already warns about the coin
acceptor's 12 V pulse line.

![Piezo impact input wiring](../figures/module-target-input.svg)

Three components condition it:

- **`R1` (1 M)** drains the charge and sets sensitivity. Lower = less sensitive.
- **`R2` (100 k)** limits current, so a 50 V spike delivers under 0.5 mA.
- **`D1`/`D2`** clamp the node to `3V3 + Vf` and `−Vf`. **Note the directions:**
  `D1` conducts *into* 3V3, `D2` conducts *from* ground *into* the node. Reversed,
  `D2` clamps the signal to +0.7 V and quietly destroys it instead of protecting
  the pin.

The GPIO's own logic threshold acts as a crude comparator. That is enough to
start. An **LM393 with a trimpot reference** turns the threshold into something
adjustable without a soldering iron, for about €0.30, and is the recommended
second iteration.

> **Starting without the diodes.** `R2` alone is enough for the bench experiment
> in step 1: the ESP32's internal protection diodes handle the residual once
> current is limited to microamps. Fine on a board you can afford to lose, not
> fine on four targets you intend to use.

**The trick worth knowing:** use the *digital interrupt for when* and a
*peak-hold for how hard*. `C1` charges to the impact peak in microseconds and
bleeds off through `R5` over tens of milliseconds -- long enough for an unhurried
ADC read *after* the interrupt has already fired. Impact strength then costs no
fast sampling at all, and `Event::Measurement` already exists to carry it.

![Peak hold wiring](../figures/module-target-peakhold.svg)

A piezo also does **not** saturate, whereas an accelerometer clips at ±16 g and a
direct impact goes well past that. For amplitude, the piezo is the better sensor.

### The parts, and how they would be wired

Starting points, not measurements -- step 1 exists to replace the values that
matter. This is here rather than in `docs/hardware/` because there is nothing to
document yet: `docs/` describes modules that exist, and when this one does, this
table is what its page starts from.

| Qty | Part | Notes |
| --- | ---- | ----- |
| 1 | ESP32 dev board | **Shock-mounted on foam.** See below. |
| 1 | Piezo disc, 27 mm, wires attached | The **passive element**, a bare brass disc — *not* an "active buzzer", which contains a driver and reads nothing. |
| 1 | `R1` resistor, 1 M | Bleeds the charge; sets sensitivity and decay. |
| 1 | `R2` resistor, 100 k | Limits the current the clamp has to sink. |
| 2 | `D1` `D2` diodes, 1N4148 | The clamp. **Not optional** past the bench. |
| 1 | Rigid plate, ~30 × 30 cm | 3–5 mm plywood or aluminium. Acrylic cracks. |
| 1 | LED strip segment, 12 V | A 5 mm LED is invisible across a room in daylight. |
| 1 | `Q1` MOSFET, IRLZ44N | **Logic-level.** A plain IRF540 will not switch properly at 3.3 V. |
| 1 | `R3` resistor, 100 R | Gate series resistor. |
| 1 | `R4` resistor, 10 k | Gate pull-down. |

| Net | Pin | Direction | Connects to | Notes |
| --- | --- | --------- | ----------- | ----- |
| `IMPACT` | `GPIO34` | input, no internal pull | `R2` from piezo node; `D1`/`D2` clamp | Input-only pin; `R1` defines the idle level. Also ADC1_CH6. |
| `LED` | `GPIO26` | output, push-pull | `R3` → `Q1` gate | High = lit. |
| `PEAK` | `GPIO35` | analog in | `C1` peak-hold node | **Optional**, only if impact strength is wanted. ADC1_CH7. |

`GPIO34`/`GPIO35` are **input-only** pins, which is a feature here: nothing can
accidentally drive a sensor line. They have no internal pull, which is fine --
`R1` holds the node at ground.

![LED driver wiring](../figures/module-target-led.svg)

`R4` is the part people leave out. While the ESP32 boots, `GPIO26` floats, and a
floating MOSFET gate can switch the strip on. `R4` holds it off until the firmware
takes control. **A simpler alternative:** a WS2812B strip needs one data pin and
no MOSFET at all, driven from `esp-hal`'s RMT peripheral. That makes the net table
three lines -- `5V`, `GND`, `DATA` -- and this schematic does not apply.

### The mechanical problems that are easy to under-budget

- **Shock-mount the ESP32.** 640 N into a frame, hundreds of times, unseats
  dupont jumpers and cracks solder joints. Foam-mount the board; solder or use
  screw terminals. `docs/HARDWARE.md`'s breadboard advice survives a finger, not
  a rally, and needs a sentence saying so.
- **Decouple the plate from the enclosure** with rubber grommets, so the shock
  goes into the plate and not into the electronics or the neighbouring target.
- **Plate material:** 3–5 mm plywood or aluminium. Rigid transmits the impact to
  the piezo. Acrylic cracks.
- **Target size:** a 10 cm target at 5 m is not a game; ~30 cm square is.
- **The LED is invisible.** A 5 mm LED at 20 mA cannot be seen across a room in
  daylight. This needs an LED strip segment or a high-power LED behind a MOSFET,
  probably at 12 V -- a BOM change, a driver transistor, and a real cost against
  [plan 10](10-power-and-battery.md). **Decide this before building four.**
- **Battery and enclosure** have to take the shock too, and a LiPo that flies
  around inside a box is a genuine hazard, not just a reliability problem.
- **An accelerometer would have to be bonded rigidly to the plate**, which
  conflicts with shock-mounting the board -- it would need its own module on the
  plate, wired back over I²C. The piezo is two wires you glue and forget.

### Cross-talk, and why the token design mostly dissolves it

Four plates on one frame: one hit rings them all.

In plan 13's rules, **`lit = (holder == me)` and only the holder's input passes
the token**, so a non-holder can ignore its sensor entirely. The game logic gates
the problem away for free.

It only returns if "hit the wrong target" should carry a penalty. Then the
options are foam-isolating each plate (free, try first), or comparing peak
amplitudes across modules over the mesh and letting the loudest win -- the beacon
has ~16 spare payload bytes for it, at the cost of a ~50 ms arbitration window.

### Latency

Hit → holder advertises → successor hears → lights is 50–200 ms over the
advertising bus. A player has to reposition and swing, so this is probably fine,
but it is tighter than a finger game and it strengthens the case for plan 13's
`ADV_IND` escape hatch (20 ms floor) over the 100 ms non-connectable one.

## Steps

### 0. Plan 14 first

Interrupt-latched inputs are a hard prerequisite. A 5 ms impact sampled from the
BLE work loop is dropped silently, and no sensor choice fixes that.

### 1. One piezo, one plank, no modules (~€5, an afternoon)

**Do this before building anything modular.** Tape a piezo to a scrap of plywood,
wire the clamp, log timestamps from an interrupt handler, and hit it.

- [ ] Every hit detected, from a gentle lob to as hard as you dare?
- [ ] How long does the plate ring? (This sets the dead time.)
- [ ] Does a hit on an adjacent plate on the same bench false-trigger it?
- [ ] Does the clamp actually protect the pin? Check with a scope if you have one.
- [ ] Fill in the table below. **Everything after this step is downstream of
      those four numbers.**

### 2. Turn the answers into constants

- [ ] Set the dead time from the measured ringing -- expect 150–300 ms, not 50.
      A ball rebounding off a wall and returning is a *real* second hit and must
      not be swallowed; ringing must be.
- [ ] Set the threshold (resistor value, or trimpot) from the gentle-lob case.
- [ ] Decide whether amplitude is worth a peak-hold, or whether binary is enough.

### 3. Build one target module

- [ ] New firmware crate `crates/module-target`, or a feature of
      `module-button` -- decide once step 1 says how different it really is.
- [ ] Report `Event::Pressed { channel: 0 }`, plus `Event::Measurement` if
      amplitude was worth it.
- [ ] `Role::Target` in `shared::proto` if the role genuinely differs from
      `Button`. It may not.
- [ ] The bright light, with its driver.
- [ ] Give it a page in `docs/hardware/`, starting from the parts and nets above,
      and add its row to `docs/HARDWARE.md` and to the capability table in
      `docs/COMPOSING.md` -- which a test will then check against the firmware.

### 4. Build three, and play it

- [ ] Three targets, plan 13's token firmware, no laptop.
- [ ] Measure press-to-light end to end and record it in plan 13's table.
- [ ] Play for an hour. Count missed hits and false triggers -- **both, by hand.**

### 5. Decide

- [ ] Is it fun, and is the latency acceptable?
- [ ] Piezo forever, or move to the LIS3DH for software-tunable thresholds?
- [ ] Does the shock-mounting hold up over a session, or does something work loose?

## Done when

- [ ] A target detects every hit from a gentle lob upward, with no missed hits
      over an hour of play.
- [ ] A hit on a neighbouring target does not trigger it.
- [ ] The lit target is visible across the room in daylight.
- [ ] Nothing has broken, worked loose, or needed re-seating after that hour.
- [ ] `docs/hardware/` documents the module that was actually built, including
      the clamp, and the design above has been deleted from this plan rather than
      left to disagree with it.
- [ ] The measurements below are filled in.

## Notes

### Why not just a very rugged button

Arcade buttons are built for a thumb: ~1 N actuation, tens of newtons of
survivable load. The gentlest ball hit here is ~200 N mean and ~320 N peak. And
even setting force aside, a 67 mm deformable ball does not reliably depress a
plunger -- it wraps around it. Two independent reasons, either sufficient.

### Deliberately not decided here

- **Whether the target is also a button.** A module that can be hit *and* pressed
  would let one box serve both game types. Probably easy, definitely not worth
  designing before one target exists.
- **Outdoor use.** Sunlight rules out unmodulated IR and makes the LED problem
  much worse. The README already promises outdoor operation; this is a place that
  promise gets tested.
- **Scoring by impact strength.** The plumbing (`Event::Measurement`, spare
  beacon bytes) exists. What it should *mean* is a game design question.

### Measurements

Step 1 exists to fill this in. Do not build modules before it is complete.

| What | Expected | Measured |
| ---- | -------- | -------- |
| Piezo output, gentle lob (clamped) | ~1–3 V | |
| Piezo output, hard hit (clamped) | clamped at ~3.9 V | |
| Plate ringing duration after one hit | 10–50 ms | |
| Dead time that gives exactly one event per hit | 150–300 ms | |
| Adjacent plate on the same bench: false trigger? | yes, unless isolated | |
| Missed hits per 100, gentle lobs | 0 | |
| Current draw of the bright LED | | |
