# Hardware

What one modit button module is made of, and how to wire it.

## Bill of materials — per module

| Qty | Part | Notes |
| --- | ---- | ----- |
| 1 | ESP32 dev board | Plain ESP32 (Xtensa). The firmware targets `xtensa-esp32-none-elf`; an ESP32-C3/C6 is a *different* architecture and will not work unmodified. |
| 1 | Momentary push button | Any normally-open tactile switch. |
| 1 | LED | Any colour. |
| 1 | Resistor, 220 Ω – 1 kΩ | In series with the LED. Not optional — the GPIO will drive far more current than the LED survives. |
| 1 | USB cable | Must be a *data* cable. A charge-only cable is the single most common "the board does not appear" cause. |

A breadboard and jumper wires for the first one; solder for the ones that go in a box.

## Pinout

Defined in [`crates/module-button/src/main.rs`](../crates/module-button/src/main.rs).

| Signal | Pin | Configuration |
| ------ | --- | ------------- |
| Button | **GPIO33** | Input, internal **pull-down**. Reads high while pressed. |
| LED    | **GPIO26** | Output, push-pull. High = lit. |

## Wiring

```
     ESP32
   +---------+
   |         |
   |  3V3 o--+------o  o------+
   |         |     button     |
   |         |                |
   | GPIO33 o-----------------+
   |         |    (internal pull-down holds this low
   |         |     until the button is pressed)
   |         |
   | GPIO26 o------[ 220Ω ]------|>|------+
   |         |                    LED     |
   |         |                          -----
   |  GND  o--------------------------- GND
   +---------+
```

Two things that catch people out:

- **The button goes to 3V3, not GND.** The pin is configured with a pull-down, so
  it idles low and pressing pulls it *up*. Wiring the button to GND gives a pin
  that is always low and a module that never notifies.
- **The LED's long leg (anode) faces the resistor / GPIO26.** Backwards, it simply
  never lights, with no other symptom.

## Choosing different pins

Change `peripherals.GPIO33` / `peripherals.GPIO26` in
[`crates/module-button/src/main.rs`](../crates/module-button/src/main.rs) and
update this file. If you move the button to a pin with no internal pull-down, add
an external one or invert the logic — `InputConfig::default().with_pull(Pull::Down)`
is doing real work.

Avoid GPIO6–11 (connected to the on-board flash) and GPIO34–39 (input-only, so
they cannot drive an LED).

## Powering a module away from a laptop

Any 5 V USB source works: a phone charger, a USB battery pack. The module only
needs power — all communication is over BLE.
