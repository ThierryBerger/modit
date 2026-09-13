# 16 — Survive the Bluetooth stack dying

**Audit:** new (2026-09-07, from a real run) · **Goal:** fail-free run · **Size:** M
· **Closes [plan 04](../doing/04-fail-free-run.md) chaos row 5 · subsumes
[plan 09](../doing/09-scenario-seam.md)'s outstanding per-module recovery**

## Why

A laptop went to sleep mid-game with three modules bound. On wake:

```
btleplug::corebluetooth::peripheral > Event receiver died, breaking out of corebluetooth device loop.
brain::link::ble  > timed out asking 00:00:00:00:00:00 whether it is connected
brain::link::ble  > module a disconnected
brain::scenarios  > module a dropped out; continuing without it
brain::scenarios  > module b hit
brain::scenarios  > module c is lit
...
```

CoreBluetooth's session died, which invalidates **every** peripheral handle at
once. The brain noticed for exactly one module, kept the other two, and carried
on running a game against boards it could no longer talk to. It never recovered,
and it never will: nothing in the current design brings a module back while at
least one stream is still nominally alive.

Plan 04's chaos checklist already has this row — "turn the laptop's Bluetooth off
and on → logged, recovers or exits with one clear sentence". Sleep is that row.
It does neither. It logs one module, stays up, and reports rounds that may not
have happened.

The goal of this project is a run nobody has to babysit. A game that keeps
scoring against absent hardware is worse than one that stops, because the log
says everything is fine.

## Current state

**Loss is only ever detected per module, by polling.**
`ModuleStream::next` (`link/ble.rs:99-131`) has two detectors: the notification
stream ending, and a 1 s `is_connected()` poll. There is no adapter-level
detector at all. Plan 04's target state named `Central::events()` and
`CentralEvent::DeviceDisconnected` as the right mechanism; it was never done, and
this incident is what that omission costs.

**The liveness check reports something, and it is not evidence.**
`is_connected` (`link/ble.rs:134-155`) logged `timed out asking
00:00:00:00:00:00`. That looked like proof the peripheral handle was already
dead. It is probably not: **CoreBluetooth does not expose MAC addresses**, so
btleplug identifies peripherals by UUID on macOS (`PeripheralId(3441db0c-…)` is
what a scan prints) and `BDAddr` is a placeholder that reads as zeros whether the
peripheral is healthy or not. The line was noise dressed as a clue.

Now fixed to name the module instead, which is the thing a person reading a game
log can act on. **The underlying question is still open**: what, on macOS, tells
you a peripheral handle has been invalidated? `is_connected()` timing out for `a`
while returning something usable for `b` and `c` says the answer is not this.

**`lost` is terminal.**
`ModuleStream.lost` (`link/ble.rs:95`) is set once and never cleared, and lost
streams are filtered out of every subsequent poll (`link/ble.rs:171`). A module
that drops is gone for the rest of the process.

**Recovery is all-or-nothing.**
`BleRx::recv` returns `None` only when *every* stream is lost
(`link/ble.rs:163-165`). That `None` is the sole route back to `acquire`: it
breaks the pump, which closes the event channel, which gives the scenario
`WaitError::Closed`, which ends it, which makes `run` re-acquire
(`runtime.rs:303-315`). So losing 3 of 3 modules recovers in about a second, and
losing 1 of 3 never recovers at all. The incident hit the second case.

**A command to a dead module cannot fail.**
`ble/write.rs` writes with `WriteType::WithoutResponse` — there is no
acknowledgement, so a write into a husk returns `Ok(())`. `set_output` on a
module that is not there succeeds. This is why lighting a lost module does not
end the round: the runtime has no failure signal on the command path whatsoever.

**Scenarios hold a stale module list.**
`Modules::ids()` (`runtime.rs:60`) is a snapshot taken at acquisition and never
shrinks. `whack_a_mole` copies it once (`scenarios.rs:47`) and keeps picking from
it, so a module known to be lost stays in the pool: the round lights a box that
is not there and then waits the full 30 s `PATIENCE` for a press that cannot
come.

**The log cannot answer what happened.**
`main.rs` builds the logger with `pretty_env_logger::formatted_builder()`, which
emits no timestamps. In the incident log, `b` and `c` alternate hits with no
timeouts between them. Whether that was someone playing normally on two surviving
modules, or a runaway loop firing rounds as fast as it could, **cannot be
determined from the log** — and that ambiguity is itself a bug.

## Target state

Two distinct failure modes, currently conflated:

| Failure | Correct response |
| ------- | ---------------- |
| One module goes away (battery, range) | demote it, keep playing with the rest, re-acquire it in the background |
| The adapter/session dies (sleep, BT toggled) | every handle is invalid — tear down and re-acquire the whole bench |

Plus the invariants that make both observable:

- A scenario can ask which modules are **live**, and a shrinking bench is a
  normal thing rather than an error.
- Commanding a module that is not there is an error the runtime can see.
- The log carries timestamps, so a post-mortem is possible.

## Steps

Ordered so each is separately testable, and so the diagnosis lands before the fix.

- [x] **Timestamps first.** `formatted_builder` → `formatted_timed_builder` in
      `main.rs`. One line, and it is what makes every step below diagnosable.
      Without it the next step produces another unreadable log. **Done.**
- [x] **Name the module in the liveness log** rather than an address that is
      always zeros on macOS (`link/ble.rs`). **Done** — and see the correction in
      Notes, because the address was originally read as evidence.
- [ ] **Reproduce deliberately.** With `just brain --modules a,b,c` running:
      `sudo pmset sleepnow`, wake, capture. Then separately toggle Bluetooth off
      and on. Record what each of the three modules does in each case — they may
      not behave alike, and the incident suggests they do not.
- [ ] **Observe the adapter.** Subscribe to `Central::events()` and log every
      `CentralEvent` at `info`. **Read-only, no behaviour change** — the point is
      to find out what btleplug actually reports on macOS wake before designing
      around it. `DeviceDisconnected` may or may not arrive; the answer decides
      the next step.
- [ ] **Find a detector that works on macOS.** The all-zero address is not one.
      Candidates, in order of cost: the `CentralEvent` stream from the step
      above; `peripheral.is_connected()` returning `Err` rather than timing out;
      a write that fails. **Establish what each does on a healthy peripheral
      first** — a detector that fires when nothing is wrong is worse than none.
- [ ] **Add link-level death.** Either a `ModuleEvent::LinkDown` or `BleRx::recv`
      returning `None` when the adapter session is gone, so `run` re-acquires the
      whole bench. On macOS wake this is the *correct* response — every
      CoreBluetooth handle is invalid, so per-module recovery cannot work.
- [ ] **Make loss survivable per module.** Clear `lost` by re-acquiring in the
      background: a demoted module rejoins when it reappears. This is plan 09's
      outstanding item, and the incident is the argument for finally doing it.
- [ ] **Expose liveness to scenarios.** `Modules::live_ids()` alongside `ids()`,
      and `whack_a_mole` picks its target from the live set. A lost module must
      never be lit.
- [ ] **Decide the write question.** `WithoutResponse` means commands can never
      fail; `WithResponse` gives an ack and costs a round trip per LED write.
      Measure the latency on real hardware before choosing — an LED that lights
      10 ms later does not matter, and knowing the write landed does.
- [ ] **Re-run plan 04's chaos checklist**, rows 1, 2, 4 and 5, now that there
      are three boards on the desk.

## Done when

- [ ] Laptop sleeps and wakes with the game running: within ~10 s all three
      modules are bound again and the round continues. No restart, no manual
      step.
- [ ] Bluetooth toggled off and on: same.
- [ ] One module powered off mid-game: it stops being chosen as a target, the
      other two keep playing, and it rejoins on its own when repowered.
- [ ] A module that is not there is never lit — no round ever waits out
      `PATIENCE` on a box that is known to be gone.
- [ ] The log has timestamps and names every demotion and every re-acquisition,
      so the sequence above can be read back afterwards.
- [ ] Plan 04's chaos row 5 can be ticked.

## Notes

- **The `00:00:00:00:00:00` address is not a clue.** It was read as one at first:
  "btleplug could not even answer where the peripheral was, so the handle must be
  dead." Then a healthy scan turned out to print `PeripheralId(<uuid>)` for every
  device, because CoreBluetooth has no MAC addresses to give — the zeros are
  what that field always looks like on macOS. The lesson is the plan's own first
  step in miniature: an unfamiliar value in a log is not evidence until you have
  seen what it looks like when everything is fine.
- Do not assume the `b`/`c` hits in the incident were phantom. Without
  timestamps that is unknowable, which is exactly why the first step is
  timestamps and the second is a deliberate reproduction rather than a fix.
- macOS sleep is not the same event as a module walking out of range, and the
  temptation will be to handle it with the same code path. It is the whole-link
  case, and treating it as three independent module losses is what produced this
  incident.
