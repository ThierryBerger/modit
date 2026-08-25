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

- [x] Write `docs/HARDWARE.md` with the pinout and a wiring diagram.
- [x] Write `docs/TUTORIAL.md` following the outline.
- [x] Trim the README's "How to" and link out.
- [x] Add the architecture notes.
- [~] **Follow the tutorial on a clean machine, or at minimum a clean shell and a
      fresh clone.** Every step that needed improvisation is a bug — fix the code,
      then fix the doc.
      **PARTIALLY DONE — host steps only. The hardware steps have never been
      walked. This is the main outstanding item; see CHECKME.md.**
- [x] Add CI that runs `cargo check` on both workspaces, `cargo fmt --check`, and
      `cargo clippy -- -D warnings`. Building the firmware in CI needs the `esp`
      toolchain — `esp-rs/xtensa-toolchain` is the usual action.
- [~] Add the `--simulate` mode from plan 04 if it is not done, and give the
      tutorial a "try it without hardware" step 0.
      **NOT DONE — it needs plan 09's seam. The tutorial says so explicitly rather
      than pretending otherwise.**

## Done when

- [~] A fresh clone plus the tutorial produces a working game with no source
      reading. **Unverified — needs boards.**
- [~] Every command in the tutorial is copy-pasteable and was actually run.
      **Host commands yes; `just flash <id>` and everything downstream, no.**
- [~] Every log line quoted in the tutorial matches what the code emits.
      **Derived from the source, not observed. Flagged in CHECKME.md.**
- [~] CI is green on `main`. **Workflow written; `just ci` passes locally. Never
      run on GitHub — there is no remote configured.**

## Notes

Written 2026-08-25. **Written, not walked.** Read the verification caveat below
before trusting it.

### What was written

| File | Contents |
| ---- | -------- |
| `docs/TUTORIAL.md` | Ten steps, each ending in something observable, plus a troubleshooting table keyed by symptom. |
| `docs/HARDWARE.md` | BOM, pinout, ASCII wiring diagram, the two mistakes that produce silent failures. |
| `docs/ARCHITECTURE.md` | The crate split and why, the two traits and why, where UUIDs live, the shape of a round, known rough edges. |
| `.github/workflows/ci.yml` | Two jobs: host (fmt, clippy, test) and firmware (fmt, clippy, **build**). |

The README now points at all three and no longer tries to be the manual.

### Deliberate choices

**Step 5 exists to bisect failures.** "Confirm it advertises, with a phone" is not
strictly necessary to get a working game, but it is the single point that
separates "firmware problem" from "host problem". Everything after it can be
diagnosed as host-side. Worth its place even though it needs a second device.

**The troubleshooting table is keyed by symptom, not by cause.** You look things
up by what you can see, which is the only thing you have when you are stuck.

**CI builds the firmware rather than checking it.** `cargo check` does not link,
and the panic handler from plan 06 resolves at link time — `custom_halt` could go
missing and `check` would not notice. `just ci` was updated to match, so local and
CI agree.

**The tutorial is honest about plan 09.** Step 10 ("write your own scenario") says
plainly that BLE plumbing and game rules are still interleaved and that writing a
scenario today means reading some btleplug. Overselling that step is how a
tutorial loses a reader's trust for everything else in it.

### The verification caveat

Steps 1, 3 and 8 were run. Steps 2 and 4–7, and 9, involve a board and were not.

**The log lines quoted in step 8 were derived by reading the source, not by
observing a successful bind.** They should be right — the format strings are a few
lines away in `ble/mod.rs` — but "should be right" is exactly the kind of claim
this project already had too much of. They are flagged in `CHECKME.md` as the
first thing to correct when a board is on the desk.

The tutorial itself carries a status note saying the same thing, so a reader is
not misled by it before it has been walked.

### CI is unrun

There is no git remote on this repository, so the workflow has never executed. The
`esp-rs/xtensa-toolchain` action version in particular is worth checking on first
push.
