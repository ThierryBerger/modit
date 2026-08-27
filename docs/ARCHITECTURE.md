# Architecture

What to read when you want to *change* modit rather than run it.

## The three crates

```
                shared  (no_std)
                   |
      module definitions + the well-known UUIDs
                   |
        +----------+----------+
        |                     |
   module-button          brain
   ESP32 firmware         host binary
   no_std, bleps          tokio + btleplug
        |                     |
        +---- BLE (GATT) -----+
```

| Crate | Runs on | Toolchain |
| ----- | ------- | --------- |
| [`shared`](../crates/shared) | both | host (`no_std` when built for a board) |
| [`brain`](../crates/brain) | your laptop | stable |
| [`module-button`](../crates/module-button) | an ESP32 | `esp`, target `xtensa-esp32-none-elf` |

## Why there are two workspaces

`module-button` needs a different toolchain *and* a different target *and*
`build-std`. Cargo resolves features and picks one target per workspace, so the
firmware cannot be a member of the host workspace. It carries its own
`[workspace]` table, `Cargo.lock`, `rust-toolchain.toml` and `.cargo/config.toml`.

`just check` builds both; the root `Cargo.toml` explains the split at the point
where someone would try to "fix" it.

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

## Where the UUIDs live

[`shared::uuids`](../crates/shared/src/lib.rs) is the single source of truth.
`brain` references it directly.

The firmware cannot: `gatt!` parses UUIDs at macro-expansion time and generates
its handle identifiers from them, so it only accepts string *literals*. Those
literals are therefore duplicated, and a host-side test
(`firmware_uuids_match_shared`) reads the firmware source and fails if they drift.
This is the mechanism that replaced the old `assets/` files, which drifted
silently.

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

### The three waiting primitives

```rust
next_event(timeout)          -> (ModuleId, Event)   // everything
next_press(timeout)          -> (ModuleId, u8)      // any module, presses only
wait_for_press(id, timeout)  -> u8                  // one module
```

Each is a filter over the one above. That order matters: a scenario needing "the
next press, whichever module" — Simon Says — cannot be built from a per-module
wait, while the per-module wait is trivially built from it.

Prefer `next_press` to hand-filtering `next_event`. An ignore arm in a `match`
still consumes the caller's loop iteration, which is a real bug that shipped
briefly here.

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
always ends the round. But the runtime does not yet re-acquire a lost module in
the background, so recovery is still *coarse* — see
[plan 09](../plans/doing/09-scenario-seam.md).

## Known rough edges

**The protocol lives in the UUIDs rather than in a type.** A module type *is* a
set of UUIDs, so adding one means minting UUIDs and hardcoding them on both sides,
and a prop with more than one input or output cannot be represented at all. The
drift test described above exists because there is no shared definition to compare
against — [plan 11](../plans/todo/11-message-protocol.md) replaces it with typed
`Command`/`Event` messages in `shared`.

Those two are the same seam from opposite sides and are best done together.

See [`plans/AUDIT.md`](../plans/AUDIT.md) for everything else that is known and
unfixed.
