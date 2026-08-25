# 05 — Give each module a stable identity

**Audit:** 5, 26 · **Goal:** fail-free run · **Size:** medium

## Why

Every board advertises the same name (`modit-esp32-2`) and the same service UUID.
`brain` builds its module list in discovery order, so "button 0" is a different
physical box on every run. A scenario cannot say "light the red button" — only
"light whichever button was found first this time".

This is also what blocks re-acquisition in plan 04: when a module drops and comes
back, there is no way to know it is the *same* module.

## Current state

- `buttons/src/main.rs:85` — `format!("modit-{}-2", esp_hal::chip!())`. The `-2` is
  hand-edited per board before flashing.
- `ble/mod.rs:100` — `brain` matches on `local_name.starts_with("modit")`.
- `ble/mod.rs:86-93` — de-duplication is by `peripheral.id()`, so two boards do not
  collide, but which slot each lands in is arbitrary.
- `ble/mod.rs:116-127` — first module definition that matches a peripheral claims it.

## Target state

A module announces **what it is** and **which one it is**, and `brain` binds by
identity rather than by discovery order.

Options, in increasing order of effort:

1. **Name suffix from a compile-time env var.** `modit-<role>-<id>`, e.g.
   `modit-button-a`. Set via `env!("MODIT_ID")` at flash time:
   `MODIT_ID=a cargo run --release`. This is almost certainly what the abandoned
   `dotenvy_macro` dependency and gitignored `.env` were for (audit 23) — the
   mechanism was started and never finished.
2. **Derive from the chip's MAC / eFuse.** Zero configuration, stable forever, but
   the ids are opaque hex and you need a registry mapping MAC → "red button".
3. **A read characteristic exposing a config blob** (role, id, firmware version,
   capabilities). Most flexible, most work, and the natural home for the currently
   dead `rf3` placeholder at `buttons/src/main.rs:103-109`.

**Recommendation: 1 now, 3 later.** Option 1 is a few lines and unblocks plan 04
immediately. Option 3 is the right long-term answer and `rf3` is already the slot
for it.

## Steps

- [x] Decide the naming scheme and write it in the README. Suggested:
      `modit-<role>-<id>` where role ∈ {`button`}, id is a short lowercase token.
- [x] Firmware: read the id from `env!("MODIT_ID")` at compile time; fail the build
      with a clear `compile_error!` if unset, rather than defaulting silently.
- [x] Firmware: replace the hardcoded `-2` at `buttons/src/main.rs:85`.
- [x] Add `.env.example` documenting `MODIT_ID`. `dotenvy_macro` was **not**
      re-added: `option_env!` is a compile-time read and needs no crate.
- [x] `shared`: add an `id: &'static str` (or the typed equivalent from plan 06) to
      the module definitions.
- [x] `brain`: match peripherals by parsed name rather than `starts_with("modit")`,
      and bind each definition to the peripheral with the matching id.
- [x] `brain`: log clearly when a `modit-*` peripheral is seen whose id is in no
      scenario — that is the "I flashed the wrong id" case, and it should say so.
- [x] `brain`: error at startup if two module definitions share an id.
- [x] Document the flash-per-board procedure in the README (plan 08 consumes this).
- [x] Settle the `buttons` crate rename deferred here by plan 02.

## Done when

- [~] Flashing two boards with different `MODIT_ID`s and running `brain` binds them
      to the same scenario slots on every run, in any power-on order.
      **Needs hardware.** Verified indirectly: `MODIT_ID=a` and `MODIT_ID=b`
      produce binaries with different hashes.
- [~] Powering a module off and on re-binds it to *its own* slot, not the first free
      one. **Needs hardware.**
- [x] Flashing without `MODIT_ID` fails at compile time with a readable message.
      **Verified.**
- [x] A `modit-*` device with an unknown id produces a log line naming the id.

## Notes

Done 2026-08-25. Option 1 from the plan (name suffix from a compile-time env var).

### Firmware

`MODIT_ID` is read with `option_env!` in a `const` initialiser, so a missing value
is a **compile** error carrying real advice:

```
error[E0080]: evaluation of constant value failed
  --> src/main.rs:63:13
   |
63 |       None => panic!(
64 | |         "MODIT_ID is not set, so this board would be indistinguishable from
             every other one. Flash with `just flash <id>` ...
```

No `dotenvy_macro` needed — that dependency (removed in plan 01) is not coming back.

**The important part is in `build.rs`:** `cargo:rerun-if-env-changed=MODIT_ID`.
Without it cargo does not know the binary depends on the variable, so flashing
board `b` right after board `a` would silently reflash `a`'s firmware from cache —
two boards advertising the same name, which is the exact bug this plan exists to
fix. Verified: changing the id recompiles, and the two binaries hash differently.

The board also prints its own identity on boot, which is the first thing you want
from a serial monitor.

### Brain

Binding is now by identity rather than discovery order. This needed a trait split:

- `ModuleDefinition<T>` — implemented by the *parts* of a module (`Notifier`,
  `Writable`). These have no identity; they only know how to find a
  characteristic.
- `Module<T>: ModuleDefinition<T>` — a whole module, one per physical board, and
  the only thing `init_bluetooth` will accept. It adds `advertised_name()`.

That split is what stops the identity concept leaking into types that cannot
honestly answer it.

`init_bluetooth` now looks up the module whose `advertised_name()` equals the
peripheral's local name, and binds only that one. Three new diagnostics fall out:

- a `modit-*` board no module expects → *"is a modit device but no module in this
  scenario expects it -- flashed with the wrong id?"*
- a board with the right name but the wrong characteristics → *"advertises the
  right name but does not expose the characteristics module 0 needs -- stale
  firmware?"*
- two modules sharing an id → a startup error, before the radio is touched.

The waiting message now names boards rather than kinds:

```
WARN brain > waiting for 2 module(s): module 0 (modit-button-a), module 1 (modit-button-b); rescanning
```

### Crate renamed

Plan 02 deferred the `buttons` rename to here, and with a role vocabulary in place
the right name is obvious: **`crates/buttons` → `crates/module-button`**, matching
the `modit-<role>-<id>` scheme. It scales to `module-nfc`, `module-motor` and so
on, and it is singular because one crate is one kind of module, not a collection.

### Still open

`MODIT_ID` is a free-form string. There is no check that the id you flashed is one
the scenario knows about *until* the board is powered on and the brain logs the
"wrong id?" warning. A registry in `shared` would catch it earlier — worth it only
once there is more than one module type.
