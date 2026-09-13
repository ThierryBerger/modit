# Hardware

What a modit module is made of, and how to wire it.

| Module | What | Status |
| ------ | ---- | ------ |
| **[module-button](hardware/module-button.md)** | One button, one LED. | Works on real hardware. **Start here** — this is what the [tutorial](TUTORIAL.md) uses. |
| **[module-coin](hardware/module-coin.md)** | A coin acceptor. **Involves 12 V.** | **Arduino Uno: works on real hardware**, standalone -- counts coins, does not talk to the brain yet. ESP32: work in progress, firmware written, never met the acceptor. |

Each page opens with what the module is *for* before it says what to solder, so
this table is also the answer to "which one do I build?".

New to this? Go straight to [module-button](hardware/module-button.md) and then
[the tutorial](TUTORIAL.md). If the question is what a *game* can do with these
rather than how to build one, that is [composing a game](COMPOSING.md), which
carries the same three modules as a table of channels a scenario can address.

## Before you wire anything to 12 V

The coin acceptor runs on 12 V. Neither the ESP32 nor the Arduino Uno does.

**Measure every wire on the acceptor against `GND` before you connect it to
anything.** These units ship with four or five wires whose colours and labels
vary between clones, and 12 V on a GPIO destroys the pin — silently, and you
will spend the next hour blaming BLE.

What you are sorting them into: the pulse output can go to a pin (it is
open-collector, so the board's pull-up sets its idle level), while `COUNTER`
drives a 12 V tally counter and stays unconnected.
[module-coin](hardware/module-coin.md) has the circuit and the reasoning.

## How this is documented

Each module page carries a **net table**, a generated schematic, and the
reasoning. The net table is the source of truth, and a test in `brain` checks it
against the firmware's pin constants so it cannot silently go stale.

The conventions, the designator scheme, and how to regenerate the drawings are in
[`hardware/README.md`](hardware/README.md).

## Choosing different pins

Change the pin in the firmware's `main.rs`, then update that module's net table —
`cargo test -p brain wiring_tables_match_firmware` will tell you if you forgot.

Constraints on any ESP32 pin choice:

| Pins | Constraint |
| ---- | ---------- |
| `GPIO6`–`GPIO11` | Connected to the on-board flash. **Never use these.** |
| `GPIO34`–`GPIO39` | Input-only, and have **no internal pull-up or pull-down**. Cannot drive an LED; ideal for a sensor with its own bias resistor. |
| `GPIO0`, `GPIO2`, `GPIO12`, `GPIO15` | Strapping pins — the level at boot selects the boot mode. Usable, but a pull-up or a pressed button here can stop the board booting. |

If you move an input to a pin with no internal pull, add an external resistor or
invert the logic. `InputConfig::default().with_pull(Pull::Down)` is doing real
work on `module-button`.

## Powering a module away from a laptop

Any 5 V USB source works: a phone charger, a USB battery pack. A module only
needs power — all communication is over BLE.

The exception is [module-coin](hardware/module-coin.md), which additionally needs
a 12 V supply sized for the acceptor's ~350 mA solenoid spike. On the Arduino Uno
that supply powers the Uno too, and **USB stays unplugged whenever the 12 V supply
is in**, so a wiring mistake cannot reach the computer.

Battery life is a firmware question far more than a radio one, and this firmware
never sleeps, so a module draws roughly the same current idle as in play.
