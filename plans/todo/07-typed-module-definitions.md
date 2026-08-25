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

- [ ] Make the decision above and record it in the Notes section below.
- [ ] `shared`: introduce a UUID type usable from `no_std`. `uuid` with
      `default-features = false` works on both sides; a `[u8; 16]` newtype with a
      `const fn` parser also works and avoids the dependency on the firmware.
- [ ] `shared`: add `const` definitions for the well-known service and characteristic
      UUIDs, with doc comments explaining the convention (why one service, why the
      LED/button characteristic UUIDs differ only in their first byte).
- [ ] `brain`: build module definitions from those constants instead of literals.
- [ ] `buttons`: reference the same constants from the `gatt!` invocation. Note that
      `gatt!` takes string literals — this may need a `const`-to-literal shim or a
      compile-time assertion that the literal matches the constant. **A test that
      fails when they drift is worth more than a clever unification.**
- [ ] Add that drift test: a unit test in `shared` asserting the firmware's literals
      and `brain`'s constants agree.
- [ ] Seed the RNG from entropy (`SmallRng::from_os_rng()`), with an optional
      `--seed` flag for reproducible debugging.
- [ ] Remove the two `// TODO: use proper type` comments once they are true.

## Done when

- An invalid UUID cannot be constructed — the error is a compile error or a
  startup error naming the module, never a mid-scan panic.
- Changing a UUID in one place and not the other fails a test.
- `cargo test` at the repo root passes and covers at least the drift test.
- Two consecutive runs of `brain` produce different button sequences.

## Notes

_(record the RON decision here — this is the section future-you will read first)_
