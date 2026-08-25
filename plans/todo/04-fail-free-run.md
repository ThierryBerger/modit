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

- [ ] `is_connected()`: bind the select result and honour it —
      `Ok(connected) => connected`, `Err(e) => { warn!(...); false }`, timeout => `false`.
      Verify by powering off a board mid-run and seeing the disconnect logged.

**Then the notification path:**

- [ ] Move `peripheral.notifications()` out of `read_notification` and into module
      binding. Store the stream in `IdentifiedModule` (or hand it to the task).
- [ ] Delete `read_notification`; replace the poll loop with `select!` over the
      held stream and a shutdown signal.
- [ ] Filter notifications by characteristic UUID. The current code notes it
      "only subscribes to one characteristic so it's fine to skip this check"
      (`read.rs:15-16`) — that stops being true the moment a second notifier exists.

**Then error tolerance:**

- [ ] Replace the write `unwrap()`s with a helper that logs and reports failure
      rather than panicking, so the caller can demote the module.
- [ ] Add a `ModuleState { Bound, Lost }` and re-acquire lost modules in the
      background rather than tearing down the whole run.
- [ ] Add exponential backoff to the acquire retry (3 s → 30 s cap) so a genuinely
      absent module does not spin the radio forever.

**Then the concurrency papercuts:**

- [ ] Drop the RNG lock before sleeping in the re-arm task (`main.rs:135-148`), or
      pre-compute the delay and target index while holding it, then release.
- [ ] Replace the `for t in tasks { join!(t) }` loop with
      `futures::future::join_all` and log any `JoinError`.
- [ ] Guard `buttons.is_empty()` before the modulo at `main.rs:91`.

**Then prove it:**

- [ ] Write down a manual chaos checklist (see below) and run it.
- [ ] Consider a `--simulate` mode with no radio at all, so the scenario logic can
      be tested on a laptop with no hardware. This is the single highest-value thing
      for picking the project back up — see plan 08.

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

- Every row of the chaos checklist passes.
- `grep -rn 'unwrap()' crates/brain/src` shows nothing in the run phase.
- A button press is never dropped because it landed between polls (verify: press
  100 times, count 100 notifications in the log).

## Notes

_(fill in while doing)_
