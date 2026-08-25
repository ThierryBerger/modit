# 07 — Make module definitions real (typed UUIDs, loadable assets)

**Audit:** 6, 21 · **Goal:** straightforward to get into · **Size:** medium

## Why

`shared` models a UUID as `&'static str` with two `// TODO: use proper type`
comments (`shared/src/lib.rs:9,16,28`). That single choice causes three problems:

1. Invalid UUIDs are representable, so they fail at scan time deep inside a loop
   rather than at construction (audit 12).
2. `&'static str` cannot be produced by runtime deserialization, which is exactly
   why the RON workflow the README promises was never finished.
3. The definitions have to be duplicated by hand between `brain` and the firmware,
   and they have already drifted once.

## Current state

- `shared/src/lib.rs` — `Notifier { service: &'static str, charac_notify_id: &'static str }`
  and `Writable { service: &'static str, charac_write_id: &'static str }`.
- `brain/src/main.rs:57-66` — UUIDs hardcoded inline.
- `buttons/src/main.rs:112,117,123` — the same UUIDs hardcoded again, in string
  literals inside the `gatt!` macro.
- `assets/` — deleted by plan 02 for being stale and unused.
- `brain/src/main.rs:88` — `SmallRng::seed_from_u64(42)`, so every run is identical.

## Target state

A UUID type that cannot hold an invalid value, one source of truth for the
well-known UUIDs, and — if the RON workflow is still wanted — definitions that
actually load.

### Decision to make first

**Is the RON workflow still wanted?** Two honest answers:

- **No — constants are enough.** Put the well-known UUIDs in `shared` as `const`s
  and have both crates reference them. Deletes the drift problem entirely, costs
  nothing, and a scenario is plain Rust. **Recommended** — it matches the README's
  "moddable means writing custom Rust".
- **Yes — non-programmers should edit configs.** Then the definitions must be
  owned values (`uuid::Uuid`, or `heapless::String` on the firmware side), loaded
  either via `include_str!` at compile time or from a path at runtime.

Do not build the RON path unless there is a concrete reason. It was already
abandoned once.

## Steps

- [x] Make the decision above and record it in the Notes section below.
- [~] `shared`: introduce a UUID type usable from `no_std`.
      **NOT DONE — see the decision in Notes. `&'static str` stays for now.**
- [x] `shared`: add `const` definitions for the well-known service and characteristic
      UUIDs, with doc comments explaining the convention (why one service, why the
      LED/button characteristic UUIDs differ only in their first byte).
- [x] `brain`: build module definitions from those constants instead of literals.
- [x] `buttons`: reference the same constants from the `gatt!` invocation. Note that
      `gatt!` takes string literals — this may need a `const`-to-literal shim or a
      compile-time assertion that the literal matches the constant. **A test that
      fails when they drift is worth more than a clever unification.**
- [x] Add that drift test: a unit test in `shared` asserting the firmware's literals
      and `brain`'s constants agree.
- [x] Seed the RNG from entropy (`SmallRng::from_os_rng()`), with an optional
      `--seed` flag for reproducible debugging.
- [x] Remove the two `// TODO: use proper type` comments — they described the RON
      workflow that has now been decided against.

## Done when

- [x] An invalid UUID cannot be constructed *in practice* — there is now one
      definition site, and a startup error names the module. Not a *type-level*
      guarantee; see the decision below.
- [x] Changing a UUID in one place and not the other fails a test.
      **Verified by deliberately nudging one character and watching it fail.**
- [x] `cargo test` at the repo root passes and covers the drift test. 13 tests.
- [x] Two consecutive runs of `brain` produce different button sequences.
      *(Done during plan 04 — `SmallRng::from_os_rng()`.)*

## Notes

Done 2026-08-25.

### THE DECISION: no RON. Constants in Rust.

**Do not rebuild the config-file workflow.** Reasons, so this does not get
relitigated:

1. It was started once and abandoned, and the files left behind (`assets/`) had
   silently drifted out of agreement with the code — the exact failure a config
   file is supposed to prevent.
2. The README's own framing is *"moddable here refers to writing custom Rust"*. A
   scenario is Rust. Its UUIDs may as well be.
3. There is no non-programmer in the loop. The person editing the scenario is the
   person compiling it.

`shared::uuids` now holds `SERVICE`, `LED_WRITE` and `BUTTON_NOTIFY` with doc
comments explaining the convention. `brain` references them, so its module
definitions have no literals at all.

### The firmware cannot use those constants, and that is a real constraint

`gatt!` parses UUIDs *when it expands* — it calls `.value()` on a `LitStr` and
generates the handle identifiers (`button_charac_handle` and friends) from them.
Passing a `const` produces "cannot find value `button_charac_handle` in this
scope", which is a confusing way to learn this.

Three approaches were tried before settling:

| Approach | Outcome |
| -------- | ------- |
| `uuid: SERVICE_UUID` (a const) | fails — `gatt!` needs a literal token |
| `const _: () = assert!(str_eq(...))` next to the literals | compiles, but only checks *its own copy* against `shared` — editing the `gatt!` literal alone still slips through |
| a host-side test that reads the firmware source | catches it |

The third is what shipped: `firmware_uuids_match_shared` in `brain` reads
`crates/module-button/src/main.rs`, extracts every `uuid: "..."` from it and
compares against `shared::uuids` in order. It reads the file rather than linking
the crate because the firmware is built for a different target with a different
toolchain and cannot be a test dependency.

**Verified that it actually fails:** changing one character of one firmware UUID
produces

```
assertion `left == right` failed: the firmware and shared::uuids have drifted apart
  left: "927312e0-2354-11eb-9f10-fbc30a62cf99"
 right: "927312e0-2354-11eb-9f10-fbc30a62cf30"
```

A test that has never been seen to fail is not evidence of anything.

The test is coupled to the shape of the `gatt!` block (it expects exactly three
`uuid:` lines, in order). Its failure message says so, which is the mitigation.

`shared::str_eq` — a `const fn` string comparison — was written for the
abandoned second approach and kept: it is small, tested, and the natural tool if
a future bleps version does accept constants.

### Typed UUIDs: deliberately not done

The plan suggested replacing `&'static str` with a real UUID type. Deferred, on
the grounds that it now buys very little:

- There is exactly **one** definition site for each UUID, and `brain` references
  it rather than retyping it. A typo has nowhere to enter.
- A malformed constant is caught by `every_well_known_uuid_parses` at
  `cargo test`, and by `validate_modules` at startup with a message naming the
  module and field.
- The firmware would still need literals regardless, so a typed representation
  would sit *alongside* the literals rather than replacing them — more machinery,
  same drift surface.

Worth revisiting if `shared` ever grows definitions that are not compile-time
constants. It no longer blocks anything, because the RON workflow it used to block
has been decided against.

### `shared` builds `std` under test

`#![cfg_attr(not(test), no_std)]`, so the constants can be tested on the host
while the crate stays `no_std` for the board.
