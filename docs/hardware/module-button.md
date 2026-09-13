# module-button — one button, one LED

The original modit module, and the one the [tutorial](../TUTORIAL.md) is written
against. Firmware: [`crates/module-button`](../../crates/module-button/).

## What it is for

One thing a player can hit, and one light telling them to hit it. That is the
beat most games are made of: the game says *now, there*, a player answers, and
the game times or scores the answer.

A handful of these is already a game. Put four on a table and it is whack-a-mole;
put ten across a pitch as cones and it is a reaction drill; give one to each team
and it is a quiz buzzer. Nothing about the module changes between those -- only
the scenario does, which is the point of [composing](../COMPOSING.md).

It is a button, so it is a *slow* input: a press lasts 50-200 ms. An input
briefer than a finger -- a ball against a plate, say -- is a different sensor and
a different front end, not a different scenario.

**This is the one that works on real hardware.** Start here even if the module
you eventually want is a different one: it is a breadboard and five minutes, and
the firmware tells you whether you wired it correctly.

## Bill of materials

| Qty | Part | Notes |
| --- | ---- | ----- |
| 1 | ESP32 dev board | Plain ESP32 (Xtensa). An ESP32-C3/C6 is a *different architecture* and will not work unmodified. |
| 1 | `SW1` momentary push button | Any normally-open tactile switch. |
| 1 | `D1` LED | Any colour. |
| 1 | `R1` resistor, 220 R – 1 k | **Not optional.** The GPIO will drive far more current than the LED survives. |
| 1 | USB cable | Must be a *data* cable. A charge-only cable is the most common "the board does not appear" cause. |

Breadboard and jumper wires are fine here: a finger does not shake anything
loose.

## Nets

| Net | Pin | Direction | Connects to | Notes |
| --- | --- | --------- | ----------- | ----- |
| `BUTTON` | `GPIO33` | input, internal pull-down | `SW1` to `3V3` | Rising edge = press. Latched by interrupt. |
| `LED` | `GPIO26` | output, push-pull | `R1` → `D1` anode; `D1` cathode → `GND` | High = lit. |

![Wiring for module-button](module-button.svg)

## How it works

The pin is configured with an **internal pull-down**, so it idles low and
pressing the button pulls it *up* to 3V3. That is why `SW1` goes to `3V3` and
not to `GND`, and it is the single most common wiring mistake on this module.

The press is caught by a **GPIO interrupt**, not by polling. The BLE work loop's
period is unbounded -- it does HCI I/O whose duration depends on radio traffic --
so anything sampled there can be missed. A finger press lasts 50–200 ms and
would survive; a fast input would not. The rules live in `shared::input`, where
they are tested on the host.

## Two mistakes that produce silent failures

- **The button goes to 3V3, not GND.** Wired to GND, the pin is always low and
  the module simply never notifies. No error, nothing in the log.
- **`D1`'s long leg (anode) faces `R1` and GPIO26.** Backwards, the LED never
  lights and there is no other symptom.

Both used to look like BLE problems, several steps later. The self-test below
catches each of them directly.

## Checking it

You do not have to eyeball this. The firmware runs a **self-test at boot**,
before any BLE: it blinks the LED three times and reports whether the button pin
reads low at rest.

```
just flash a
just monitor
```

Expected:

```
self-test: blinking the LED three times (GPIO26)
self-test: button (GPIO33) reads low at rest, as expected
self-test: press the button now -- you should see a line for each press
```

A `WARNING -- button (GPIO33) reads HIGH at rest` line means the button is wired
to GND. Step 5 of the [tutorial](../TUTORIAL.md) walks through the rest.
