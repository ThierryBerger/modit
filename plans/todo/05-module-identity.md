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

- [ ] Decide the naming scheme and write it in the README. Suggested:
      `modit-<role>-<id>` where role ∈ {`button`}, id is a short lowercase token.
- [ ] Firmware: read the id from `env!("MODIT_ID")` at compile time; fail the build
      with a clear `compile_error!` if unset, rather than defaulting silently.
- [ ] Firmware: replace the hardcoded `-2` at `buttons/src/main.rs:85`.
- [ ] Add `.env.example` documenting `MODIT_ID`, and re-add `dotenvy_macro` *only*
      if it is actually used (plan 01 removes it as unused — this may re-add it).
- [ ] `shared`: add an `id: &'static str` (or the typed equivalent from plan 06) to
      the module definitions.
- [ ] `brain`: match peripherals by parsed name rather than `starts_with("modit")`,
      and bind each definition to the peripheral with the matching id.
- [ ] `brain`: log clearly when a `modit-*` peripheral is seen whose id is in no
      scenario — that is the "I flashed the wrong id" case, and it should say so.
- [ ] `brain`: error at startup if two module definitions share an id.
- [ ] Document the flash-per-board procedure in the README (plan 08 consumes this).

## Done when

- Flashing two boards with different `MODIT_ID`s and running `brain` binds them to
  the same scenario slots on every run, in any power-on order.
- Powering a module off and on re-binds it to *its own* slot, not the first free one.
- Flashing without `MODIT_ID` fails at compile time with a readable message.
- A `modit-*` device with an unknown id produces a log line naming the id.

## Notes

_(fill in while doing)_
