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

## The round

1. **Validate** — parse every UUID, reject duplicate ids. Before the radio is
   touched, so a typo is reported as a typo.
2. **Acquire** — scan, connect, bind every module by name. Retries with backoff
   (3s → 30s), never fatal.
3. **Run** — one watcher task per module, each holding that module's notification
   stream for the whole round and selecting over `{notification, round aborted,
   liveness tick}`.

Any failure ends the round and returns to step 2. Recovery is therefore *coarse*:
one wobbly board interrupts the whole game rather than just itself. Making it
per-module is [plan 09](../plans/todo/09-scenario-seam.md).

## Known rough edges

**Game rules and BLE plumbing are interleaved** in `brain`'s `main()`. There is no
seam to write a scenario against, and no way to run one without hardware —
[plan 09](../plans/todo/09-scenario-seam.md).

**The protocol lives in the UUIDs rather than in a type.** A module type *is* a
set of UUIDs, so adding one means minting UUIDs and hardcoding them on both sides,
and a prop with more than one input or output cannot be represented at all. The
drift test described above exists because there is no shared definition to compare
against — [plan 11](../plans/todo/11-message-protocol.md) replaces it with typed
`Command`/`Event` messages in `shared`.

Those two are the same seam from opposite sides and are best done together.

See [`plans/AUDIT.md`](../plans/AUDIT.md) for everything else that is known and
unfixed.
