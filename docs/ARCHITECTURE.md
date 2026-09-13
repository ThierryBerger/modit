# Architecture

What to read when you want to *change* modit rather than run it.

## The four crates

```
                    shared  (no_std)
                        |
     the wire protocol, the UUIDs, the edge-latch rules
                        |
     +---------------+--+------------+
     |               |               |
module-button    module-coin      brain
ESP32 firmware   ESP32 firmware   host binary
no_std, bleps    no_std, bleps    tokio + btleplug
     |               |               |
     +---------------+--- BLE (GATT) -+
```

| Crate | Runs on | Toolchain |
| ----- | ------- | --------- |
| [`shared`](../crates/shared) | both | host (`no_std` when built for a board) |
| [`brain`](../crates/brain) | your laptop | stable |
| [`module-button`](../crates/module-button) | an ESP32 | `esp`, target `xtensa-esp32-none-elf` |
| [`module-coin`](../crates/module-coin) | an ESP32 | the same |

## Why there are three workspaces

A firmware crate needs a different toolchain *and* a different target *and*
`build-std`. Cargo resolves features and picks one target per workspace, so
neither firmware can be a member of the host workspace, and they cannot share one
with each other either — each carries its own `[workspace]` table, `Cargo.lock`,
`rust-toolchain.toml` and `.cargo/config.toml`.

`just check` builds all three; the root `Cargo.toml` explains the split at the
point where someone would try to "fix" it.

## How a module is identified

Every board runs identical firmware, so identity is injected at flash time:
`MODIT_ID=a` becomes part of the advertised name `modit-<role>-<id>`, e.g.
`modit-button-a`.

`brain` binds by that name, not by discovery order, so module 0 is the same
physical box on every run and a module that reconnects returns to *its own* slot.

`build.rs` declares `cargo:rerun-if-env-changed=MODIT_ID` — without it, flashing
board `b` right after `a` reuses the cached binary and both boards advertise `a`.

## The two traits

Found in [`crates/brain/src/ble/mod.rs`](../crates/brain/src/ble/mod.rs):

- **`ModuleDefinition<T>`** — implemented by the *parts* of a module
  (`shared::Notifier`, `shared::Writable`). A part knows how to find its
  characteristic on a peripheral and how to validate its own UUIDs. It has no
  identity.
- **`Module<T>: ModuleDefinition<T>`** — a whole module, one per physical board.
  Adds `advertised_name()`. Only a `Module` can be bound by `init_bluetooth`.

The split exists so identity cannot leak into types that have no honest answer
for it.

## Where the protocol lives

[`shared::proto`](../crates/shared/src/proto.rs) defines what a module can be told
(`Command`) and what it can report (`Event`), plus the `Descriptor` it introduces
itself with. Adding a module type means adding variants there, not minting UUIDs.
Messages are postcard-encoded and must fit a default-MTU packet — 20 bytes — which
a test enforces.

[`shared::uuids`](../crates/shared/src/lib.rs) holds the characteristic UUIDs.
`brain` references them directly; the firmware cannot, because `gatt!` parses
UUIDs at macro-expansion time and generates its handle identifiers from them, so
it only accepts string *literals*. Those literals are therefore duplicated, and
two mechanisms stop the copies drifting: `module-coin` compares them at compile
time with a `const` assertion, and a host test (`firmware_uuids_match_shared`)
reads `module-button`'s source. This is what replaced the old `assets/` files,
which drifted silently.

### Two wire formats coexist

`module-coin` speaks `Command`/`Event` over one write and one notify
characteristic. `module-button` still exposes a button-specific pair
(`BUTTON_NOTIFY`, `LED_WRITE`) carrying a bare byte, and the BLE link synthesises
a `Descriptor` on its behalf so the runtime above it sees one kind of module.

Expect that asymmetry when reading [`link/ble.rs`](../crates/brain/src/link/ble.rs):
it is the reason the link is still typed to buttons, and the reason a coin acceptor
runs under `--simulate` but not over the radio.

## The layers

```
scenarios.rs   game rules      async fn(&Modules) -- no BLE types at all
runtime.rs     the runtime     owns the link, pumps events, hands out `Modules`
link/          the transport   Link trait: ble.rs (real) or sim.rs (no radio)
ble/           BLE plumbing    scanning, connecting, notification streams
```

A scenario never mentions a peripheral, a characteristic or a task. `Modules` is
concrete rather than generic over the transport: one task owns the link and
communicates over channels, so no type parameter leaks into scenario code.

### The waiting primitives

`next_event` is the one primitive; `next_press`, `next_coin` and `wait_for_press`
are filters over it, each narrower than the last. That order matters: a scenario
needing "the next press, whichever module" — Simon Says — cannot be built from a
per-module wait, while the per-module wait is trivially built from it.

Prefer a filter to hand-matching `next_event`. An ignore arm in a `match` still
consumes the caller's loop iteration, which is a real bug that shipped briefly
here.

The full vocabulary, and which scenario shape each call suits, is in
[composing a game](COMPOSING.md) — this page is about where the code lives, not
about how to use it.

### Running without hardware

`just simulate` swaps `link/ble.rs` for `link/sim.rs` — in-process channels
carrying the same messages. Scenario, runtime and protocol are all the real ones.
It is also the test harness: the scenario tests in `scenarios_tests.rs` drive it
programmatically.

## The round

1. **Validate** — parse every UUID, reject duplicate ids. Before the radio is
   touched, so a typo is reported as a typo.
2. **Acquire** — scan, connect, bind every module by name. Retries with backoff
   (3s → 30s), never fatal.
3. **Run** — the scenario, over held notification streams.

A lost module surfaces to the scenario as `WaitError::ModuleLost`, and the
scenario decides: whack-a-mole carries on if it was not the lit one, Simon Says
always ends the round. Recovery is **coarse**: nothing re-acquires that board
mid-scenario, so a module comes back when the scenario ends and acquisition runs
again.

## What is planned, and why it is not here

Everything known-but-unfixed lives in [`plans/`](../plans/README.md), one file
per piece of work, with the reasoning that chose it. This page describes the code
as it is — if the two ever disagree, this page is the one that is wrong.
