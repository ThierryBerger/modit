# 03 — Error messages that tell you what to do

**Audit:** 11, 12, 13, 14, 15, 16 · **Goal:** good error messages · **Size:** medium

## Why

Today the failure modes are: a panic with a btleplug backtrace, or a wall of
`println!` that repeats every 3 seconds without saying what is wrong. Neither tells
you whether the problem is a powered-off board, a Bluetooth adapter that needs
permission, or a typo in a UUID. When you come back to this after six months and it
does not work, this is the difference between five minutes and an evening.

The rule to apply: **an error message names the thing that failed, the value that
caused it, and the next thing to check.**

## Current state

- `unwrap()` on every BLE call: `brain/src/main.rs:69,70,73,84,95,129,146`;
  `ble/mod.rs:34,49,97,132`.
- `Uuid::parse_str(self.charac_notify_id).unwrap()` (`ble/mod.rs:34`, `:49`) runs
  once per characteristic per peripheral, inside the scan loop. A typo panics with
  no mention of which module it came from.
- `"No Bluetooth adapters found"` (`ble/mod.rs:68`) prints and continues.
- `"BLE peripheral devices were not found, sorry. Exiting..."` (`ble/mod.rs:82`)
  does not exit.
- `brain/src/main.rs:75-78` retries without naming the missing module.
- `pretty_env_logger::init()` at `brain/src/main.rs:56`, then zero `log` calls —
  everything is `println!`, so `RUST_LOG` does nothing.
- `dbg!(adapter_list)` at `ble/mod.rs:73`.

## Target state

- `anyhow` is already a dependency and unused — put it to work. Every fallible
  function returns `Result`, with `.context("…")` naming the module and the UUID.
- UUIDs are parsed **once**, at startup, before any radio work. A bad UUID fails
  immediately with the module name and the offending string, not mid-scan.
- Every `println!` becomes `log::{error,warn,info,debug,trace}` at an appropriate
  level. Per-characteristic chatter is `trace!`. Connection events are `info!`.
- `main` returns `anyhow::Result<()>` so the top-level error prints its context
  chain instead of a backtrace.

## Steps

- [x] Convert `println!`/`eprintln!` to `log` macros throughout `brain`. Levels:
      - `trace!` — per-characteristic inspection (`ble/mod.rs:31,48`)
      - `debug!` — scan start/stop, service discovery
      - `info!` — peripheral found, connected, module bound, game events
      - `warn!` — a module missing this round, a retry
      - `error!` — adapter missing, connect failed
- [x] Delete `dbg!` at `ble/mod.rs:73`.
- [x] Set a default filter so a bare `cargo run` is useful without `RUST_LOG` set:
      `pretty_env_logger::formatted_builder().parse_default_env().filter_level(Info)`.
- [x] Replace the two `Uuid::parse_str(...).unwrap()` with parsing at construction.
      Best done as part of plan 06 (typed UUIDs in `shared`); until then, parse once
      in `init_bluetooth` and `.context()` it with the module index and the string.
- [x] Make `init_bluetooth` return `Err` — not a printed message — when
      `adapter_list.is_empty()`, with text naming the platform-specific cause:
      on macOS, Bluetooth permission for the terminal; on Linux, `bluetoothd`.
- [x] Delete the misleading "Exiting..." message; replace with a `warn!` that says
      how many peripherals were seen and that it will rescan.
- [x] Make the retry at `brain/src/main.rs:75-78` name the unbound modules:
      `warn!("waiting for {n} module(s): {names}; rescanning in 3s")`.
- [x] `main() -> anyhow::Result<()>`; replace the remaining `unwrap()`s with `?`
      plus `.context()`. Note that the *game loop* unwraps
      (`main.rs:84,129,146`) should become retries, not `?` — that is plan 04.
- [x] Sweep: `grep -rn 'unwrap()\|expect(' crates/brain/src` should only show
      cases with a written justification.

## Done when

- [x] `RUST_LOG=warn cargo run` shows only problems. **Verified.**
- [x] `RUST_LOG=trace cargo run` shows the characteristic-by-characteristic matching.
      **Verified.**
- [x] Running with Bluetooth off produces one sentence naming Bluetooth, not a
      backtrace. **Covered by a unit test** (`no_adapters_explains_what_to_check`)
      rather than by toggling the machine's Bluetooth.
- [x] Corrupting one character of a UUID in a module definition produces an error
      naming that module and that string, before any scan starts. **Verified by
      hand and locked in by a test.**
- [x] `grep -rn 'println!|eprintln!|dbg!' crates/brain/src` is empty. **Verified.**

## Notes

Done 2026-08-25.

### Levels, as they came out

```
$ cargo run                       # no RUST_LOG
 INFO  brain > 1 Bluetooth adapter(s) available
 WARN  brain > waiting for 2 module(s): module 0 (button-led), module 1 (button-led); rescanning
```

That two-line output is the whole point of the plan: it used to be a silent
3-second cycle forever.

### Validation happens before the radio

Rather than parsing UUIDs inside the scan loop, `ModuleDefinition` grew two
methods: `label()` (a short name for messages) and `validate()` (parse every UUID
this definition holds). `validate_modules()` runs over all of them, and
`init_bluetooth` calls it first thing so it cannot be skipped. `main` also calls
it explicitly before creating the `Manager`, so a typo costs you nothing:

```
Error: invalid module definition

Caused by:
    0: module 0 (button-led)
    1: led
    2: charac_write_id is not a valid UUID: "927312e0-2354-11eb-9f10-fbc30a62cf3"
    3: invalid group length in group 4: expected 12, found 11
```

The context chain reads outside-in: what I was doing, which module, which field,
what exactly is wrong. `Uuid::parse_str` is now also called once per peripheral
instead of once per characteristic per peripheral.

`with_peripheral` still parses, but logs and returns `None` instead of
unwrapping -- so a caller that somehow skipped validation degrades to "no match"
rather than panicking mid-scan.

### Two things found while doing this

1. **A `continue` that lied.** Moving the LED-reset failure from `.unwrap()` to a
   logged retry, the obvious `continue` continues the *inner* `for` over buttons,
   not the round -- so the message would have said "restarting" while quietly
   skipping one module and playing on with a stale board. The session loop is now
   labelled `'session` and all three restart paths say `continue 'session`.

2. **`Adapter`'s `Debug` is another `dbg!`.** The first version logged
   `{adapter:?}` at debug level, which dumps every peripheral the adapter has
   cached -- pages of output, including the names of nearby phones. Replaced with
   `adapter.adapter_info()` (prints `CoreBluetooth`). For the same reason
   `IdentifiedModule` got a hand-written `Debug` that shows the peripheral *id*
   rather than deriving one that dumps the whole `Peripheral`.

### Not done here, on purpose

The game-loop write failures at what used to be `main.rs:129,146` now log an
error and abort the round instead of panicking. That is better than before --
a panic inside `task::spawn` killed only that task, and `let _ = join!(t)`
discarded the `JoinError`, so a dead task was invisible -- but it is still not
the retry behaviour plan 04 calls for. They carry `TODO(plan-04)` markers.

`join_all` now replaces the `for t in tasks { join!(t) }` loop, and a `JoinError`
is logged with the module index rather than discarded.

`read_notification` was left structurally as-is (it still reopens the stream every
poll) and carries a `TODO(plan-04)`; fixing it properly is that plan's core.

### Tests

Five, all in `ble::tests`, covering the message content rather than just the
error type -- they assert that the rendered `{:#}` chain names the module, the
field and the offending value. `cargo test` at the root runs them; there is no
hardware involved.
