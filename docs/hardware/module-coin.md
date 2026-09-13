# module-coin — a coin acceptor

A CH-92x-family coin selector, driven by one of two boards:

- an **Arduino Uno** (or a clone such as the Kuman Uno). Firmware:
  [`crates/module-coin-uno`](../../crates/module-coin-uno/). **Works on real
  hardware**: it counts coins and blinks each one's pulse count. It **cannot
  talk to the brain yet**: that is work in progress.
- an **ESP32** -- wireless over BLE. Firmware:
  [`crates/module-coin`](../../crates/module-coin/). **Work in progress**:
  written, never run against an acceptor.

Sections that depend on the board have a tab for each. On GitHub they are shown
one after the other.

> **This module involves 12 V, and neither board does.**
>
> Only one acceptor wire carries a signal to the board, and it is safe to connect
> because of how it works:
>
> - `COIN` is an **open-collector output**. It can only pull down, so *you*
>   choose its idle voltage -- pulled up to the board's own supply, 12 V never
>   appears on it.
> - `COUNTER` is **not** a signal for the board. It drives a 12 V tally counter.
>   Leave it unconnected.
>
> On an unfamiliar unit, measure every wire against `GND` before connecting
> anything: clone units relabel and recolour these wires freely. And **join the board's `GND` to the
> 12 V supply's `GND`** -- without a shared reference the COIN signal means
> nothing.

## What it is for

Taking money, or taking a token. The acceptor recognises a coin and reports it;
the brain decides what a coin buys.

The beat it adds is *the game will not start until someone pays*.

The acceptor has no inhibit input, so it cannot refuse a coin mid-game; a
scenario may need to keep coins inserted during a round as credit for later.

Concretely: an arcade cabinet at a conference stand, a fairground game, a
fundraiser where each play is a euro. The included `arcade` scenario is exactly
this -- a coin buys a round of whack-a-mole -- and it is the only scenario that
uses two *kinds* of module, so it is also the worked example for
[composing](../COMPOSING.md).

What a coin is *worth* is not wired into the module. The firmware reports the
pulses the acceptor emitted; a scenario turns pulses into credits, because
pricing is policy and the scenario is the part you edit.

**Counting coins is confirmed on hardware**, on the Uno: every coin arrives as one
burst of exactly its taught pulse count, repeatably. Playing `arcade` with a real
coin is not -- that needs the ESP32 and BLE, so today `arcade` runs under
`just simulate --scenario arcade`. Status for every module is in
[Hardware](../HARDWARE.md). What the bench established is in [plan
12](../../plans/done/12-coin-module-uno.md#confirmed-on-the-bench-2026-09-13);
what is left, in [its ESP32 half](../../plans/doing/12-coin-module-esp32.md).

## Before wiring either board

These hold for the acceptor, whichever board reads it. On the unit this page was
built against (a CH-92x-family acceptor branded 616) they are confirmed on the
bench.

- **The wires are `DC12V`, `COIN`, `GND` and `COUNTER`**, plus a two-pin `SET`
  header. On a clone whose labels you do not trust, check with a meter against
  the acceptor's `GND`, acceptor on 12 V: `DC12V` reads 12 V, and `COIN` floats
  (an open collector with nothing pulling it up yet).
- **Leave `COUNTER` and the `SET` pins unconnected.** `COUNTER` drives a
  mechanical tally counter; `SET` is for programming the acceptor.
- **Set the NO/NC switch to NO.** On NC the COIN line idles low and pulses high,
  the opposite of what the firmware listens for. The firmware's self-test catches
  this: it is the first cause listed when the pulse line reads low at rest.
- **Leave the pulse-speed switch where it is**, or re-check the 300 ms burst gap
  if you move it. It is confirmed at the 616's current setting, and chosen to work
  at every setting, but that second part is arithmetic from the datasheet.

## Bill of materials

<div class="tabs" data-group="board">

### Arduino Uno

| Qty | Part | Notes |
| --- | ---- | ----- |
| 1 | Arduino Uno or clone | ATmega328P, 5 V. Needs the barrel jack: it runs from the acceptor's supply. |
| 1 | Coin acceptor, CH-92x family | Programmed via the button on the unit itself. |
| 1 | 12 V PSU, 5.5 × 2.1 mm plug, 1 A or more | Feeds the acceptor *and* the Uno. The acceptor's solenoid draws a ~350 mA spike. |
| 1 | DC splitter or barrel-to-screw-terminal adapter | So one supply feeds both, each with its own return wire. |
| 1 | `R1` resistor, 10 k | Pulls `COIN` up to 5 V. |
| 1 | `R2` resistor, 10 k | In series with `D2`. Insurance, not function. |
| 1 | USB cable | For flashing only. **Never plugged in while the 12 V supply is.** |

### ESP32

| Qty | Part | Notes |
| --- | ---- | ----- |
| 1 | ESP32 dev board | As [module-button](module-button.md). |
| 1 | Coin acceptor, CH-92x family | Programmed via the button on the unit itself. |
| 1 | 12 V PSU | Sized for the acceptor's solenoid, which draws a ~350 mA spike. |
| 1 | `R1` resistor, 1 k | In series with `COIN`. Insurance, not function. |

</div>

## Nets and wiring

<div class="tabs" data-group="board">

### Arduino Uno

| Net | Pin | Direction | Connects to | Notes |
| --- | --- | --------- | ----------- | ----- |
| `COIN` | `D2` | input, **no** internal pull-up | `R2` → the acceptor's `COIN` wire, with `R1` from that wire to `5V` | Idles high; each pulse pulls it low. `D2` is `INT0`. |
| `LED` | `D13` | output | the on-board `L` LED | Blinks once per pulse of a recognised coin. |

The acceptor's `COUNTER` and `SET` pins are not connected.

#### Power

![Uno power](module-coin-uno-power.svg)

**One 12 V supply for both.** The acceptor's `DC12V` and the Uno's barrel jack
both come from it. The Uno accepts 7–12 V on the jack, and the barrel's sleeve is
the Uno's `GND`, so plugging it in also joins the two grounds.

**Give the acceptor's `GND` its own wire back to the supply.** The solenoid's
spike has to return to the supply's (−). Routed through the Uno's `GND` pin
instead, it lifts the Uno's idea of "low" by however much that path drops, and a
pulse gets missed.

Wired this way, the solenoid firing does not reset the Uno and does not cost a
pulse: confirmed on the bench.

**The USB cable and the 12 V supply are never connected at the same time.** USB
puts the computer on the same ground as the 12 V supply, and then a wiring
mistake on the bench can reach the computer's USB port. So: unplug the 12 V
supply, plug in USB, flash, unplug USB, plug the 12 V supply back in. The Uno is
not powered from USB during a run, and it does not need to be.

#### COIN pulse input

![Coin pulse input wiring, Uno](module-coin-uno-input.svg)

This is the circuit from the widely copied CH-926 Arduino tutorial -- a 10 k
pull-up to 5 V on the COIN wire, into `D2` -- plus `R2` in series. Built as drawn,
it counts every pulse.

`R1` sits on the **acceptor's side** of `R2`, not the pin's. In normal operation
`R2` then carries only the pin's leakage current, so `D2` reads the acceptor's
transistor directly, at about 0.2 V, however large `R2` is. That is what allows
`R2` to be 10 k: large enough that finding the 12 V wire by mistake pushes well
under a milliamp into the pin's protection diode -- `(12 − 5.5) / 10 k` -- which
the ATmega328P tolerates.

It is also why the internal pull-up stays **off**. Enabled, it is a ~35 k
pull-up on the *pin's* side of `R2`, and the low level becomes a divider:
`5 V × 10 k / 45 k ≈ 1.1 V`, uncomfortably close to the 1.5 V the ATmega reads as
low. `R1` already keeps the line defined, even with the acceptor unplugged.

`D2` because it is `INT0`, one of the Uno's two pins with a dedicated edge
interrupt. It is also the pin the tutorial uses.

### ESP32

| Net | Pin | Direction | Connects to | Notes |
| --- | --- | --------- | ----------- | ----- |
| `COIN` | `GPIO27` | input, internal pull-up | `R1` → the acceptor's `COIN` wire | Idles high; each pulse pulls it low. |

#### Power

The ESP32 runs from 5 V USB, as every module does; the acceptor from its own
12 V supply. **Join the two grounds.**

#### COIN pulse input

![Coin pulse input wiring](module-coin-input.svg)

**Work in progress.** What this relies on about the acceptor -- an
open-collector `COIN`, idling high on NO, pulses the firmware's timing splits
correctly -- is confirmed on the Uno. What is not: that the ESP32's weak internal
pull-up, through `R1`, is enough on its own.

The acceptor's COIN output is **open-collector**: a bare transistor that pulls
the line down and never drives it up. Whatever pulls it up decides its idle
voltage, and here that is the ESP32's own internal pull-up to 3.3 V. So the
wire goes to `GPIO27` and that is all it takes.

`GPIO27` idles **high**, and each coin pulse is a **falling edge** — which is
exactly what the firmware listens for.

`R1` does nothing in normal operation. It is there for the abnormal one: these
acceptors ship with four or five unlabelled wires whose colours vary between
units, and `R1` limits what happens when the wire you guessed is the 12 V one.
At 1 k that is still about `(12 − 3.8) / 1 k ≈ 8 mA` into the ESP32's protection
diode. Espressif does not rate that, so treat it as *often* survivable rather
than safe.

</div>

## What the acceptor actually reports

It does not report a value. It emits a **burst of pulses**, and the number of
pulses names the coin it recognised, as taught by the programming button on the
unit. The firmware's whole job is: count the pulses, decide the burst has ended
after 300 ms of quiet, and report the count.

What a pulse is *worth* is venue policy and lives in a scenario, not here.

## Why the pulse line is an interrupt

A pulse can be as short as ~30 ms, and a missed pulse does not merely lose an
event — it changes the *value* of the burst, turning a two-credit coin into a
one-credit coin. So the line is an edge interrupt with its own debounce, and the
main loop only ever reads a count the handler already committed to.

## Checking it

<div class="tabs" data-group="board">

### Arduino Uno

Flash it with the 12 V supply **unplugged**, USB in:

```
just flash-coin-uno
```

It opens the serial console afterwards, which should print
`self-test: D2 idles high; post a coin`. On USB alone that only proves `R1`
reaches `5V`: the acceptor is unpowered. Close the console, unplug USB, then plug
in the 12 V supply.

At boot it blinks the `L` LED three times slowly. A coin taught as *n* pulses
then blinks it *n* times, in one group. A steady fast blink instead means `D2`
read low at rest: the NO/NC switch is on NC, the grounds are not joined, or that
is not the COIN wire, and nothing is counted until it is fixed. A quick flicker
right at power-on is the bootloader, not a coin; three slow blinks again later
mean the Uno reset.

That self-test is the check: on 12 V it reads `D2` at rest, so a multimeter on
`D2` is optional.

### ESP32

**Work in progress** -- never run against an acceptor.

```
just flash-coin slot
just monitor
```

The self-test reports the pulse line's idle level. A `WARNING -- coin line (GPIO27) reads LOW at rest` means the
acceptor is unpowered, the grounds are not joined, the NO/NC switch is on NC, or
that is not the COIN wire. **Do not trust the pulse counts until it reads high.**

</div>
