# CHECKME — things I could not verify

**Temporary file.** Delete it once you have worked through it (or fold anything
still outstanding into the relevant plan and delete the rest).

Everything below compiles, passes clippy under `-D warnings`, and passes the host
test suite. None of it has touched a real ESP32. Ordered by *how much rests on
it*, not by effort.

You need: two ESP32 boards wired per [`docs/HARDWARE.md`](../docs/HARDWARE.md), a
phone with a BLE scanner app.

---

## 🔴 1. The event-loop rewrite — the big one

**Plan 04.** The round used to poll: it reopened a notification stream every
200 ms and dropped it. It is now an event loop holding one stream per module for
the whole round, selecting over notification / round-aborted / liveness-tick.

This is the largest behavioural change in the pass and the thing most likely to be
wrong. Everything else in this file is smaller.

- [ ] **Presses are never dropped.** Press one button 20 times, slowly. Count 20
      `module N hit` lines. This is the bug the rewrite exists to fix — under the
      old code, presses landing between polls vanished.
- [ ] **Fast presses too.** Press 20 times as fast as you can. Still 20.
- [ ] **A press while nothing is armed** is logged at debug and ignored, not lost
      or misattributed. (`RUST_LOG=brain=debug`)
- [ ] **The game actually progresses** — light moves, hits register, no stalls
      after a few dozen rounds.
- [ ] **Leave it running for an hour.** No task pile-up, no memory growth, no
      silent stall. The re-arm path spawns a task per hit; if that leaks, an hour
      will show it.

> If something here misbehaves, please confirm it before starting plan 09 — that
> plan builds directly on this code, and debugging both at once is miserable.

---

## 🔴 2. The panic handler actually reboots

**Plan 06.** The firmware used to have `#[panic_handler] { loop {} }` — a panic
stopped the board dead, silently, indistinguishable from a flat battery.

Now `esp-backtrace` prints the panic and a backtrace, then `custom_halt()` calls
`esp_hal::system::software_reset()`.

I verified this only at link level: the binary contains `custom_halt` as a defined
symbol and esp-backtrace's `PANIC` strings. **Whether it actually reboots on a
real chip is unverified.**

- [ ] Add a deliberate `panic!("checkme")` somewhere in the main loop, flash, and
      watch `just monitor`. Expect: the panic message, a backtrace, `rebooting
      after panic`, then the board starting up again (`modit button module, id ...`).
- [ ] **Remove the deliberate panic afterwards.**

If it prints but does not reboot, `custom-halt` is not being invoked — check the
feature is still enabled in `crates/module-button/Cargo.toml`.

---

## 🟠 3. Two boards keep their identities

**Plan 05.** Every board used to advertise the same name, so "module 0" was
whichever board answered first. Now the id is compiled in.

- [ ] `just flash a` on board 1, `just flash b` on board 2.
- [ ] A phone scanner shows **`modit-button-a`** and **`modit-button-b`**.
- [ ] `just brain` binds module 0 → a and module 1 → b.
- [ ] **Power them on in the opposite order and run again.** Module 0 must still
      be board a. This is the whole point of the plan.
- [ ] Power one off mid-round and back on: it returns to *its own* slot.
- [ ] Flash a board with `just flash zzz` (an id no scenario expects). Expect:
      `"modit-button-zzz" is a modit device but no module in this scenario expects
      it -- flashed with the wrong id?`

**Watch for a stale-cache bug specifically:** flash `a`, then immediately flash
`b`, then scan. If both boards advertise `a`, then
`cargo:rerun-if-env-changed=MODIT_ID` in `build.rs` is not working and the second
flash reused the first binary. I verified the two builds hash differently, but not
through the actual `just flash` path.

---

## 🟠 4. Debounce is right for your physical button

**Plan 06.** Debounce was counted in BLE-loop iterations (so the window stretched
with radio load); it is now a 50 ms wall-clock window, `DEBOUNCE_MS` in
`crates/module-button/src/main.rs`.

50 ms is a textbook default, not a measured value for *your* switch.

- [ ] One deliberate press → exactly one `button pressed, ...` line in the
      monitor. If you see doubles, raise `DEBOUNCE_MS`. (Easiest to check right
      after boot, before the brain connects — presses are logged either way.)
- [ ] Rapid deliberate presses still all register. If they are being swallowed,
      lower it.

---

## 🟡 5. Malformed input does not kill a board

**Plan 06.** `data[0]` and an unbounded `copy_from_slice` were both reachable from
the radio, and with the old silent panic handler either would have bricked a
module until power-cycled.

Needs a BLE client that can write raw values (nRF Connect can):

- [ ] Write a **zero-length** value to the LED characteristic
      (`927312e0-2354-11eb-9f10-fbc30a62cf30`). Expect `empty write to led_charac,
      ignoring` and a board that keeps running.
- [ ] **Read** the button characteristic. Expect zero bytes back and no panic.

Lower priority only because these need a deliberate malformed write to trigger.

---

## 🟡 6. The tutorial survives being followed

**Plan 08.** [`docs/TUTORIAL.md`](../docs/TUTORIAL.md) was written but never walked.
Steps 1, 3 and 8 were run; everything involving a board was not.

- [ ] Follow it start to finish, ideally in a fresh shell and a fresh clone.
- [ ] **Correct the log lines quoted in step 9.** They were derived by reading the
      format strings in `ble/mod.rs`, *not* by observing a successful bind. They
      should be right, but "should be right" is exactly the kind of claim this
      repo already had too much of.
- [ ] **Check the step 5 wiring self-test does what it claims** — three visible
      blinks, and the button warning firing when the button is deliberately wired
      to GND rather than 3V3. It is new firmware code, written blind.
- [ ] Fix anything you had to improvise. A step that needs improvising is a bug in
      the code, not the doc.
- [ ] Remove the note at the top of the tutorial once it has been walked.
- [ ] Check the wiring diagram in `docs/HARDWARE.md` against your actual build.

---

## ⚪ 7. CI has never been watched

`just ci` passes locally and mirrors `.github/workflows/ci.yml`, but no run of it
has been read.

- [ ] Check the `esp-rs/xtensa-toolchain@v1.5` action version is still current —
      pinned actions rot.
- [ ] Confirm the firmware job's `MODIT_ID: ci` gets through. Without it the build
      fails by design, which is easy to mistake for a broken workflow.

---

## ⚪ 9. Power draw is unmeasured

[Plan 10](todo/10-power-and-battery.md) is independent of everything else. It needs one board and a USB power meter, nothing more.

- [ ] Measure average current: advertising unconnected, connected idle, connected
      with presses.
- [ ] Do the battery arithmetic for the pack you intend to use.
- [ ] **If a day of sessions plus an overnight charge is the real requirement, the
      current firmware may already meet it.** Recording that and closing the plan
      is a good outcome, not a cop-out.

---

## ⚪ 9. The landing page has no picture of anything

[Plan 17](doing/17-documentation-per-module-and-web-ready.md) built the
documentation site and its landing page, and every claim on it is text. A page
about *physical* game props with no photograph is the weakest part of it, and it
is the one step of that plan that cannot be done at a desk.

- [ ] Photograph two wired boards. A phone on a table is enough; it does not have
      to be pretty, it has to be real.
- [ ] Record ten seconds of a round being played -- light on, hit, next light. A
      terminal log is not a demonstration of a game.
- [ ] Drop both into `docs/` and put them at the top of
      [`docs/index.md`](../docs/index.md).
- [ ] While the boards are out, check the site's claims against them: the landing
      page says the button module works, and nothing else is described as working.

## Where things stand

Which plans are done, which wait on hardware, and what to pick up next is in
[`README.md`](README.md) — it is not repeated here.

Nothing in this file is a known bug. It is all "I wrote this and could not watch
it work".

**One part of the project is now genuinely tested**: the scenario layer runs
under `just simulate` and in 8 automated tests, so game logic no longer depends
on a board to verify. The BLE path below it is still unwatched — `link/ble.rs`
is deliberately a thin wrapper over the unchanged `ble/` code so that this pass
did not add risk there.
