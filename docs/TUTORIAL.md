# Tutorial: from an empty machine to a working game

Builds two button modules and runs the built-in whack-a-mole scenario against
them. Every step ends with something you can *observe*, so a failure is localised
to one step instead of discovered at the end.

Allow about an hour the first time, most of it waiting for `espup install`.

> **Status:** the host-side steps (1, 3, 8) have been run as written. The steps
> involving a board have not yet been walked through end to end — see
> [`CHECKME.md`](../CHECKME.md). If something here is wrong, that is a bug in this
> file; please fix it as you go.

## What you need

Two ESP32 modules, wired per [`HARDWARE.md`](HARDWARE.md). You can do the whole
tutorial with **one** board and stop before step 6 — you will just not have a game
to play, because whack-a-mole needs somewhere to move to.

---

## 1. Get the repository building

```sh
git clone <this repo> && cd modit
cargo test
```

**Observe:** the tests pass. This needs no hardware and no special toolchain — it
builds only the host crates.

If `cargo` is missing, install Rust from <https://rustup.rs>.

---

## 2. Wire the first module

Follow [`HARDWARE.md`](HARDWARE.md). Button between **GPIO33** and **3V3**; LED
from **GPIO26** through a resistor to **GND**.

**Observe:** nothing yet — but check the wiring against the diagram now, because
step 5 is much harder to debug if this is wrong.

---

## 3. Install the firmware toolchain

The firmware needs Espressif's Rust fork; the ESP32's Xtensa core is not a target
upstream `rustc` supports.

```sh
just setup
```

That runs `cargo install espup espflash` and `espup install`, and then reminds you:

```sh
source ~/export-esp.sh
```

**This is per-shell.** Every new terminal in which you build firmware needs it
again. Forgetting it is the most common cause of a firmware build failing with
linker errors, and the error does not mention it.

**Observe:**

```sh
rustup toolchain list     # includes `esp`
espflash --version        # prints a version
```

---

## 4. Flash the first module

Connect exactly one board. Each board gets its own id, which is baked into the
firmware and becomes the name it advertises:

```sh
just flash a
```

**Observe:** `espflash` writes the binary and drops into a serial monitor showing

```
modit button module, id "a", on esp32
...
started advertising
```

Leave the monitor open for the next step. `Ctrl+]` exits it.

> The id is compile-time, so **each board must be flashed separately** with its
> own id. Two boards flashed `a` will collide and the brain will bind only one.

<details>
<summary>Board not detected?</summary>

- Confirm the cable carries data, not just power.
- `ls /dev/tty.*` (macOS) or `ls /dev/ttyUSB*` (Linux) should list a new device
  when the board is plugged in.
- Some boards need the **BOOT** button held while connecting.
- Linux: you may need to be in the `dialout` group.
</details>

---

## 5. Confirm it is advertising

**Do not skip this.** It is the checkpoint that separates "firmware problem" from
"host problem" for every failure after it.

Use any BLE scanner — *nRF Connect* or *LightBlue* on a phone, or `bluetoothctl`
with `scan on` on Linux.

**Observe:** a device named **`modit-button-a`** in the scan results.

If it is missing, the problem is on the board, and the serial monitor from step 4
will say why. If it is present, everything from here is host-side.

---

## 6. Flash the second module

Disconnect the first board, connect the second:

```sh
just flash b
```

**Observe:** the monitor prints `id "b"`, and a scan now shows both
`modit-button-a` and `modit-button-b`.

---

## 7. Power both modules

Both boards need power but not a laptop — any USB charger or battery pack works.

**Observe:** both names in a BLE scan, at the same time.

---

## 8. Run the brain

```sh
just brain
```

**Observe**, roughly in this order:

```
 INFO  brain     > 1 Bluetooth adapter(s) available
 INFO  brain::ble > found "modit-button-a" (module 0)
 INFO  brain::ble > connected to "modit-button-a"
 INFO  brain::ble > subscribing to characteristic 917312e0-2354-11eb-9f10-fbc30a62cf30
 INFO  brain::ble > bound module 0 (button-led a) to "modit-button-a"
 INFO  brain::ble > found "modit-button-b" (module 1)
 ...
 INFO  brain     > all 2 module(s) bound, starting the game
 INFO  brain     > module 1 is lit
```

If instead you see this repeating, the brain is running fine and simply cannot
find a board — the message names which one:

```
 WARN  brain > waiting for 1 module(s): module 1 (modit-button-b); rescanning in 3s
```

The wait grows 3s → 6s → 12s → 24s → 30s so it does not hammer the radio. It
resets as soon as everything binds.

---

## 9. Play

One LED lights. Press **that** module's button.

**Observe:**

```
 INFO  brain > module 1 hit ([78, 111, 116, 105, 102, ...])
 INFO  brain > module 0 is lit
```

Pressing the wrong one is logged and ignored:

```
 INFO  brain > module 0 pressed, but 1 was expected
```

Set `RUST_LOG` for more or less:

| Command | Shows |
| ------- | ----- |
| `RUST_LOG=warn just brain` | only problems |
| `just brain` | connections and game events (default) |
| `RUST_LOG=brain=debug just brain` | scan lifecycle too |
| `RUST_LOG=brain=trace just brain` | every characteristic considered, every peripheral skipped |

---

## 10. Write your own scenario

The game lives in `main()` in
[`crates/brain/src/main.rs`](../crates/brain/src/main.rs). To change the rules,
edit it: the module handles give you `write::write(...)` for the LED and a
notification stream per module for presses.

Be aware that BLE plumbing and game rules are still interleaved in that function.
Separating them into a runtime and a scenario is
[plan 09](../plans/todo/09-scenario-seam.md) and has not been done — so today,
writing a scenario does mean reading some btleplug. Adjust your expectations
accordingly, and consider doing plan 09 first if you plan to write several.

---

## Troubleshooting

Keyed by what you actually see.

| Symptom | Likely cause | Fix |
| ------- | ------------ | --- |
| `espflash` cannot find a port | charge-only cable, or board not in bootloader | use a data cable; hold **BOOT** while connecting; check `ls /dev/tty.*` |
| Firmware build fails with linker errors | `source ~/export-esp.sh` not run in this shell | run it; it is per-shell |
| `error: MODIT_ID is not set` | built the firmware without an id | use `just flash <id>`, not a bare `cargo run` |
| No `modit-*` device in a phone scan | firmware not running, or it panicked | check the serial monitor — a panic prints a backtrace and reboots |
| `no Bluetooth adapters found` | Bluetooth off, or permission | the error names the fix for your OS; on macOS the *terminal* needs Bluetooth permission in System Settings → Privacy & Security |
| `waiting for N module(s)` forever | that board is off, out of range, or flashed with a different id | the message names the expected board, e.g. `modit-button-b` |
| `is a modit device but no module in this scenario expects it` | board flashed with an id the scenario does not list | reflash with the right id, or add it to `modules` in `main.rs` |
| `advertises the right name but does not expose the characteristics` | board running older firmware | reflash it |
| Button press does nothing | wiring, or the brain is not subscribed | the serial monitor prints `button pressed, notifying` on every press — that isolates it to one side or the other |
| Serial monitor prints `button pressed, but no client is subscribed` | the board is fine; the brain has not bound it | see the brain's own log |
| LED never lights | wiring or LED polarity | `RUST_LOG=brain=trace` shows the write going out; if it does, it is the wiring |

## Running without hardware

Not possible yet. A `--simulate` backend is the highest-value missing piece for
picking this project back up on a train, and is part of
[plan 09](../plans/todo/09-scenario-seam.md).
