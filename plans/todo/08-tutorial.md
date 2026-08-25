# 08 — The tutorial

**Audit:** 19, 20, 22, 28 · **Goal:** straightforward to get into · **Size:** medium

## Why

This is the actual deliverable. Everything before it is groundwork so that the
tutorial can be *true* and so that following it *works*.

The test is concrete: **someone who has never seen this repo — including you in six
months — follows the document top to bottom and ends with a working two-button
game, without reading any source code.**

Write it last, but write it as the acceptance criterion for everything else. If a
step in the tutorial is embarrassing to write down, that is a bug in the code, not
in the tutorial.

## Current state

- `README.md` explains the *concept* well (why BLE, what "modit" means, the escape
  game audience) and gives zero operational information.
- The "How to" section describes a RON workflow that does not exist (audit 19).
- Nothing mentions `espup`, the `esp` toolchain, `espflash`, the target triple,
  `RUST_LOG`, or which crate to run.
- The pinout — GPIO33 button with pull-down, GPIO26 LED — exists only in
  `buttons/src/main.rs:62-69`.
- No tests, no CI.

## Target state

Split the docs by *when you read them*:

- **`README.md`** — what and why. Keep the existing concept material, it is good.
  Ends with a link to the tutorial. Trim the "How to" to a pointer.
- **`docs/TUTORIAL.md`** — the guided path from empty machine to working game.
- **`docs/HARDWARE.md`** — pinout, wiring diagram, bill of materials.
- **`ARCHITECTURE.md`** or a section in the README — the three-crate split, why
  `buttons` is a separate workspace, how `brain` binds modules. The thing you read
  when you want to change something rather than run it.

## Tutorial outline

Each numbered step must end with something the reader can *observe*, so a failure
is localised to one step rather than discovered at the end.

1. **What you need** — 2× ESP32 dev boards, 2 buttons, 2 LEDs + resistors, USB
   cables. Photo or diagram.
2. **Wire one module** — GPIO33 → button → 3V3, GPIO26 → resistor → LED → GND.
   *Observe:* nothing yet, but the wiring is checkable against the diagram.
3. **Install the toolchain** — `espup install`, `cargo install espflash`, and the
   `source ~/export-esp.sh` step that everyone forgets.
   *Observe:* `rustup toolchain list` shows `esp`.
4. **Flash the first module** — `MODIT_ID=a just flash` (from plan 05).
   *Observe:* the serial monitor prints `started advertising`.
5. **Confirm it advertises** — a phone BLE scanner app, or `bluetoothctl`.
   *Observe:* `modit-esp32-a` in the scan list. **This is the most valuable
   checkpoint in the document** — it separates "firmware problem" from "host
   problem" for every failure that follows.
6. **Flash the second module** — `MODIT_ID=b just flash`.
7. **Run the brain** — `just brain`.
   *Observe:* the exact log lines for a successful bind, quoted verbatim.
8. **Play** — what should happen, and what each log line means.
9. **Write your own scenario** — the moddable part. Where the seam is, what to
   implement, a worked example of a different game (e.g. Simon Says).
10. **Troubleshooting** — a table keyed by the *symptom the reader sees*.

## Troubleshooting table (seed content)

| Symptom | Cause | Fix |
| ------- | ----- | --- |
| `espflash` cannot find the port | board not in bootloader / missing driver | hold BOOT while connecting; check `ls /dev/tty.*` |
| Build fails with linker errors | `source ~/export-esp.sh` not run | re-run it; it is per-shell |
| No `modit-*` in a phone scan | firmware not running, or panicked | check the monitor for a backtrace (plan 06 makes this visible) |
| `brain` finds no adapters | Bluetooth off, or permission | macOS: grant Bluetooth to the terminal in System Settings → Privacy |
| `brain` waits forever on a module | wrong `MODIT_ID`, or out of range | the log names the missing id (plan 03) |
| Button press does nothing | wiring, or notify not subscribed | monitor prints `sending notif!` on press — that isolates it to the host side |
| LED never lights | wiring, or wrong write UUID | `RUST_LOG=trace` shows the write |

## Steps

- [ ] Write `docs/HARDWARE.md` with the pinout and a wiring diagram.
- [ ] Write `docs/TUTORIAL.md` following the outline.
- [ ] Trim the README's "How to" and link out.
- [ ] Add the architecture notes.
- [ ] **Follow the tutorial on a clean machine, or at minimum a clean shell and a
      fresh clone.** Every step that needed improvisation is a bug — fix the code,
      then fix the doc.
- [ ] Add CI that runs `cargo check` on both workspaces, `cargo fmt --check`, and
      `cargo clippy -- -D warnings`. Building the firmware in CI needs the `esp`
      toolchain — `esp-rs/xtensa-toolchain` is the usual action.
- [ ] Add the `--simulate` mode from plan 04 if it is not done, and give the
      tutorial a "try it without hardware" step 0. This is what makes the project
      re-enterable on a train with no boards in your bag.

## Done when

- A fresh clone plus the tutorial produces a working game with no source reading.
- Every command in the tutorial is copy-pasteable and was actually run.
- Every log line quoted in the tutorial matches what the code emits.
- CI is green on `main`.

## Notes

_(fill in while doing — especially every place you had to improvise)_
