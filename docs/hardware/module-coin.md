# module-coin — a coin acceptor

A CH-92x-family coin selector, driven by one of two boards:

- an **Arduino Uno** (or a clone such as the Kuman Uno) -- what is being brought
  up now. Its firmware is not written yet, and it **cannot talk to the brain
  yet**: that is work in progress.
- an **ESP32** -- the original design, wireless over BLE. Firmware:
  [`crates/module-coin`](../../crates/module-coin/). Written, never run against
  an acceptor.

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
> Measure every wire against `GND` before connecting anything: clone units
> relabel and recolour these wires freely. And **join the board's `GND` to the
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

**Not yet confirmed on hardware** -- `arcade` runs under
`just simulate --scenario arcade`, but no acceptor has been attached to either
board. Status for every module is in [Hardware](../HARDWARE.md), and the
bring-up is tracked in [plan 12](../../plans/doing/12-coin-module.md).

## Before wiring either board

- **Read the labels, then check them with a meter** against the acceptor's `GND`,
  acceptor on 12 V: `DC12V` reads 12 V, and `COIN` floats (an open collector
  with nothing pulling it up yet).
- **Leave `COUNTER` and the `SET` pins unconnected.** `COUNTER` drives a
  mechanical tally counter; `SET` is for programming the acceptor.
- **If the unit has an NO/NC switch, set it to NO.** On NC the COIN line idles
  low and pulses high, the opposite of what the firmware listens for.
- **Note where the pulse-speed switch is.** The 300 ms burst gap below is chosen
  to work at every setting, but it is the first thing to re-check if you move it.

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

The Uno's net table is **not checked against firmware yet** -- there is no Uno
firmware for `wiring_tables_match_firmware` to read.

#### Power

![Uno power](module-coin-uno-power.svg)

**One 12 V supply for both.** The acceptor's `DC12V` and the Uno's barrel jack
both come from it. The Uno accepts 7–12 V on the jack, and the barrel's sleeve is
the Uno's `GND`, so plugging it in also joins the two grounds.

**Give the acceptor's `GND` its own wire back to the supply.** The solenoid's
spike has to return to the supply's (−). Routed through the Uno's `GND` pin
instead, it lifts the Uno's idea of "low" by however much that path drops, and a
pulse gets missed.

**The USB cable and the 12 V supply are never connected at the same time.** USB
puts the computer on the same ground as the 12 V supply, and then a wiring
mistake on the bench can reach the computer's USB port. So: unplug the 12 V
supply, plug in USB, flash, unplug USB, plug the 12 V supply back in. The Uno is
not powered from USB during a run, and it does not need to be.

#### COIN pulse input

![Coin pulse input wiring, Uno](module-coin-uno-input.svg)

This is the circuit from the widely copied CH-926 Arduino tutorial -- a 10 k
pull-up to 5 V on the COIN wire, into `D2` -- plus `R2` in series.

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

Before any firmware, with a multimeter. Acceptor and Uno wired and on the 12 V
supply, **USB unplugged**:

- `D2` against `GND` reads **about 5 V** at rest. About 0 V means the NO/NC
  switch is on NC, the grounds are not joined, or that is not the COIN wire.
- Post a coin the acceptor was taught: it should be accepted and drop through.
  That checks the acceptor alone, before the Uno counts anything.

Once the firmware exists: at boot it blinks the `L` LED three times slowly. A
coin taught as *n* pulses then blinks it *n* times, in one group. A steady fast
blink instead means `D2` read low at rest: do not trust any counts until that is
fixed. A quick flicker right at power-on is the bootloader, not a coin.

### ESP32

```
just flash-coin slot
just monitor
```

The self-test reports the pulse line's idle level. A `WARNING -- coin line (GPIO27) reads LOW at rest` means the
acceptor is unpowered, the grounds are not joined, the NO/NC switch is on NC, or
that is not the COIN wire. **Do not trust the pulse counts until it reads high.**

</div>
