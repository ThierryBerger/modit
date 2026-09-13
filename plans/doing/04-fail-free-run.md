# 04 — A run that cannot fail

**Audit:** 1, 2, 7, 8, 9, 11, 14 · **Goal:** fail-free run · **Size:** large

## Why

The stated goal is a run that survives the real world: a board that browns out, a
module carried out of range, a laptop whose Bluetooth stack hiccups. Today any of
those is a process abort. In an escape game there is nobody at a terminal to
restart it.

"Fail-free" here means: **the only fatal errors are configuration errors, and they
happen before the game starts.** Everything after that degrades and recovers.

## Current state

- `is_connected()` (`brain/src/main.rs:40-52`) discards the actual result — a clean
  disconnect reports *connected*. The reconnect path is effectively unreachable
  except via the 1 s timeout. **Fix this first; the rest of the plan depends on
  disconnects being detectable.**
- `read_notification()` (`ble/read.rs:7-25`) opens a fresh notification stream per
  poll and drops it after 200 ms. Presses landing in the gap are lost.
- Writes unwrap (`main.rs:84,129,146`) — a device that leaves between the scan and
  the reset write aborts the process.
- `join!(t)` in a loop (`main.rs:157-159`) is a sequential await; `let _ =` discards
  the `JoinError`, so a panicked task looks identical to a finished one.
- The re-arm task holds the RNG mutex across a 500–1000 ms sleep (`main.rs:135-148`).
- `buttons.len()` could be zero at `main.rs:91` (division by zero).

## Target state

Three phases with different failure policies:

1. **Configure** — parse UUIDs, validate the scenario. Errors here are fatal and
   loud. (Plan 03 covers the messages.)
2. **Acquire** — scan and bind every module. Retries forever with backoff, logging
   which modules are still missing. Never fatal.
3. **Run** — the scenario. Any BLE error demotes the module to "lost" and returns
   to phase 2 for that module only, without killing the run.

Plus the structural fixes that make phase 3 correct:

- **Subscribe once, hold the stream.** Open the notification stream at bind time and
  keep it. Fan out with a `tokio::sync::broadcast` or give each module task
  ownership of its own stream. Never re-open per poll.
- **Event-driven, not polled.** `tokio::select!` over `{ notification, disconnect
  event, scenario timer }` rather than a 200 ms poll loop.
- **Use btleplug's event stream** (`Central::events()`) for
  `CentralEvent::DeviceDisconnected` instead of probing `is_connected()` on a timer.

## Steps

**Fix the blocker first:**

- [x] `is_connected()`: bind the select result and honour it —
      `Ok(connected) => connected`, `Err(e) => { warn!(...); false }`, timeout => `false`.
      Verify by powering off a board mid-run and seeing the disconnect logged.

**Then the notification path:**

- [x] Move `peripheral.notifications()` out of `read_notification` and into module
      binding. Store the stream in `IdentifiedModule` (or hand it to the task).
- [x] Delete `read_notification`; replace the poll loop with `select!` over the
      held stream and a shutdown signal.
- [x] Filter notifications by characteristic UUID. The current code notes it
      "only subscribes to one characteristic so it's fine to skip this check"
      (`read.rs:15-16`) — that stops being true the moment a second notifier exists.

**Then error tolerance:**

- [x] Replace the write `unwrap()`s with a helper that logs and reports failure
      rather than panicking, so the caller can demote the module.
- [~] Add a `ModuleState { Bound, Lost }` and re-acquire lost modules in the
      background rather than tearing down the whole run.
      **NOT DONE — moved to plan 09. See Notes.**
- [x] Add exponential backoff to the acquire retry (3 s → 30 s cap) so a genuinely
      absent module does not spin the radio forever. **The 30 s cap is unverified
      against a real game:** it suits a workshop, and may be too slow where a
      module that was power-cycled is expected back within seconds. Time it on
      hardware before tuning it.

**Then the concurrency papercuts:**

- [x] Drop the RNG lock before sleeping in the re-arm task (`main.rs:135-148`), or
      pre-compute the delay and target index while holding it, then release.
- [x] Replace the `for t in tasks { join!(t) }` loop with
      `futures::future::join_all` and log any `JoinError`.
- [x] Guard `buttons.is_empty()` before the modulo at `main.rs:91`.

**Then prove it:**

- [~] Write down a manual chaos checklist (see below) and run it.
      **Written; NOT run — needs hardware.**
- [~] Consider a `--simulate` mode with no radio at all, so the scenario logic can
      be tested on a laptop with no hardware. This is the single highest-value thing
      for picking the project back up — see plan 08.
      **NOT DONE — moved to plan 09, which is where the seam it needs gets built.**

## Chaos checklist

Each of these must leave the process running and recovering:

| # | Action | Expected |
| - | ------ | -------- |
| 1 | Power off a module mid-round | logged as lost, re-acquired on power-up, run continues |
| 2 | Power off *all* modules | acquire loop with backoff, no crash |
| 3 | Start `brain` before any module is powered | waits, names the missing modules |
| 4 | Walk a module out of range and back | same as 1 |
| 5 | Turn the laptop's Bluetooth off and on | logged, recovers or exits with one clear sentence |
| 6 | Press a button when nothing is expected | logged at `debug`, ignored |
| 7 | Press the wrong button | logged, round continues |
| 8 | Leave it running for an hour | no leak, no task pile-up, no stall |

## Done when

- [~] Every row of the chaos checklist passes. **Needs hardware — this is the main
      outstanding item.**
- [x] `grep -rn 'unwrap()' crates/brain/src` shows nothing in the run phase.
      **Verified — the only remaining `unwrap()` is inside a test.**
- [~] A button press is never dropped because it landed between polls.
      **Structurally fixed — there are no polls any more — but unverified on
      hardware.**

## Notes

Done 2026-08-25, apart from two items deliberately moved to plan 09.

### The blocker was already gone

`is_connected()` was fixed during plan 03 (the `select!` now binds and honours the
result instead of discarding it with `_`). Everything below assumes disconnects
are actually detectable, which they now are.

### Polling replaced with an event loop

This is the substance of the plan. Before, each watcher called
`read_notification()`, which opened a *new* notification stream, waited 200 ms and
dropped it. Presses landing in the gap were lost, and the dropped stream took any
buffered ones with it.

Now `read::notifications()` opens the stream **once**, before the round starts,
and the stream is moved into that module's watcher task and held for the whole
round. The watcher is a `tokio::select!` over three arms:

| Arm | Meaning |
| --- | ------- |
| `abort_rx.changed()` | another watcher ended the round |
| `stream.next()` | a notification arrived, or the stream ended (board gone) |
| `liveness.tick()` | nothing happened for a second — is the board still there? |

The liveness tick is now a backstop rather than the primary mechanism: a
disconnect usually shows up as the stream ending, which is immediate.

Notifications are also filtered by characteristic UUID. The old code had a
comment saying this was unnecessary because only one characteristic was
subscribed — true then, false the moment a second notifier exists, and cheap to
do properly now.

### `watch` instead of `AtomicBool`

`should_abort` was an `AtomicBool` that each watcher *polled* at the top of its
loop. That cannot work inside `select!` — there is nothing to await. It is now a
`tokio::sync::watch` channel, so a watcher blocked on its notification stream
wakes immediately when another one ends the round, instead of after the next poll.

### Backoff

Acquisition failures now back off 3s → 6s → 12s → 24s → 30s (cap), reset to zero
on a successful bind. Observed:

```
WARN brain > waiting for 2 module(s): module 0 (modit-button-a), module 1 (modit-button-b); rescanning in 3s
WARN brain > waiting for 2 module(s): module 0 (modit-button-a), module 1 (modit-button-b); rescanning in 6s
WARN brain > waiting for 2 module(s): module 0 (modit-button-a), module 1 (modit-button-b); rescanning in 12s
```

The message names both *what* it is waiting for and *when* it will try again.
Two unit tests cover the doubling and the cap.

### Also fixed here

- The RNG is seeded from the OS (`SmallRng::from_os_rng()`), so runs differ. This
  was listed under plan 07; it was a one-line change and belonged with the rest of
  the round setup.
- `buttons.is_empty()` is guarded before the modulo, so the division-by-zero path
  is closed even though `validate_modules` makes it unreachable.
- The press handler moved out of the deeply nested closure into a named
  `handle_press` function. The old version was six levels of indentation.

### Deliberately moved to plan 09

**Per-module demotion and background re-acquisition.** The plan asked for a
`ModuleState { Bound, Lost }` so one module dropping does not tear down the round.
Doing that properly means something other than the scenario owning the module
list — a supervisor that hands out handles which can be temporarily absent. That
*is* plan 09's seam, and building half of it here would be churn that plan 09 then
has to undo.

What exists instead is coarse recovery: any module failing ends the round, and the
session loop re-acquires everything with backoff. Nothing panics, nothing hangs,
and the run continues — but a wobble on one board interrupts the whole game rather
than just that module. Rows 1, 2, 3 and 4 of the chaos checklist should pass;
they will just be less graceful than they eventually should be.

**`--simulate` mode.** Same reason: it needs the backend abstraction that plan 09
builds. Noted there as the highest-value item for re-entering the project.

### Verification status

Verified here: it compiles, clippy is clean under `-D warnings`, nine tests pass,
backoff behaves as designed in a real run, and there are no `unwrap()`s left
outside tests.

**Not verified — the entire chaos checklist above needs two boards.** The
event-loop rewrite is the largest behavioural change in this pass and it has never
run against real hardware.
