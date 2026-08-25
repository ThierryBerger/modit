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

- [ ] Convert `println!`/`eprintln!` to `log` macros throughout `brain`. Levels:
      - `trace!` — per-characteristic inspection (`ble/mod.rs:31,48`)
      - `debug!` — scan start/stop, service discovery
      - `info!` — peripheral found, connected, module bound, game events
      - `warn!` — a module missing this round, a retry
      - `error!` — adapter missing, connect failed
- [ ] Delete `dbg!` at `ble/mod.rs:73`.
- [ ] Set a default filter so a bare `cargo run` is useful without `RUST_LOG` set:
      `pretty_env_logger::formatted_builder().parse_default_env().filter_level(Info)`.
- [ ] Replace the two `Uuid::parse_str(...).unwrap()` with parsing at construction.
      Best done as part of plan 06 (typed UUIDs in `shared`); until then, parse once
      in `init_bluetooth` and `.context()` it with the module index and the string.
- [ ] Make `init_bluetooth` return `Err` — not a printed message — when
      `adapter_list.is_empty()`, with text naming the platform-specific cause:
      on macOS, Bluetooth permission for the terminal; on Linux, `bluetoothd`.
- [ ] Delete the misleading "Exiting..." message; replace with a `warn!` that says
      how many peripherals were seen and that it will rescan.
- [ ] Make the retry at `brain/src/main.rs:75-78` name the unbound modules:
      `warn!("waiting for {n} module(s): {names}; rescanning in 3s")`.
- [ ] `main() -> anyhow::Result<()>`; replace the remaining `unwrap()`s with `?`
      plus `.context()`. Note that the *game loop* unwraps
      (`main.rs:84,129,146`) should become retries, not `?` — that is plan 04.
- [ ] Sweep: `grep -rn 'unwrap()\|expect(' crates/brain/src` should only show
      cases with a written justification.

## Done when

- `RUST_LOG=warn cargo run` shows only problems.
- `RUST_LOG=trace cargo run` shows the characteristic-by-characteristic matching.
- Running with Bluetooth off produces one sentence naming Bluetooth, not a backtrace.
- Corrupting one character of a UUID in a module definition produces an error
  naming that module and that string, before any scan starts.
- `grep -rn 'println!\|eprintln!\|dbg!' crates/brain/src` is empty.

## Notes

_(fill in while doing)_
