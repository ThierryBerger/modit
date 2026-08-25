# 02 — Delete the things that are not true

**Audit:** 18, 19, 25, 26, 27 · **Goal:** straightforward to get into · **Size:** small

## Why

The most expensive thing in this repo is not the missing code — it is the code and
docs that describe something that does not exist. `assets/` looks load-bearing and
is not. The README's "How to" describes a compile-time RON workflow that was never
written. `buttons/src/main.rs` opens with an esp-hal example banner claiming three
characteristics. Every one of these costs a returning reader ten minutes of
believing a false thing before the code corrects them.

Do this **before** any refactor, so the refactor starts from an honest baseline.

## Current state

| Thing | Claims | Reality |
| --- | --- | --- |
| `assets/simple_button.ron` | service `…cf38`, notify `…cf38` | firmware uses the `…cf30` family; file is read by nothing |
| `assets/simple_writable.ron` | `Buttons(service:, charac_notify_id:)` | `Writable` has `charac_write_id`; would not deserialize |
| `assets/simple_notifier.txt` | a bare UUID | no consumer |
| `assets/simple_writable.txt` | a bare UUID | no consumer, and disagrees with the `.ron` next to it |
| `README.md` "How to" | create a `.ron` per module, load at compile time | UUIDs are hardcoded at `brain/src/main.rs:57-66` |
| `buttons/src/main.rs:1-8` | "three characteristics", `//% FEATURES:`, `//% CHIPS:` | two characteristics; the markers are esp-hal's build matrix |
| `buttons/src/main.rs:85` | name `modit-esp32-2` | the `-2` is a manual per-board edit |

## Target state

Nothing in the repo describes behaviour that is not implemented. Where a design
is still wanted but unbuilt, it lives in `plans/todo/`, not in `assets/` or the
README.

## Steps

- [x] Delete `assets/` entirely. The intended design survives in plan 07; the
      broken files do not need to.
- [x] Rewrite the README "How to" to describe what the code does today. The full
      rewrite is plan 08 — for now, just stop it being wrong. A three-line
      "see plans/todo/07 for the planned asset workflow" is fine.
- [x] Replace the `buttons/src/main.rs` header with a real module doc comment:
      what the board is, what it advertises, which pins it uses.
- [x] Delete the `//% FEATURES:` / `//% CHIPS:` markers.
- [x] Add an explicit `// TODO(plan-05): per-board identity` next to the `-2` in the
      advertised name, so the hack is labelled rather than mysterious.
- [x] Add `use bleps::att::Uuid;` to the imports in `buttons/src/main.rs`, even though
      the `gatt!` macro already provides it via expansion (audit 24). An import you
      can see beats one you cannot.
- [~] Rename what is misnamed, or write down why not: the crate is `buttons` but the
      module is a button *and* an LED. `button_led` or `module-button` is honest.
      A rename touches the flash command and the README, so decide deliberately.
      **Deferred -- see Notes. Still an open decision.**

## Done when

- `grep -rn "ron\|assets" --include='*.rs' --include='*.toml' --include='*.md' .`
  returns only intentional hits.
- Every doc comment in `crates/buttons/src/main.rs` matches the code below it.
- The README makes no claim that a fresh reader could disprove in under a minute.

## Notes

Done 2026-08-25, with one step deliberately deferred.

- `assets/` deleted outright. All four files were unreferenced, and three of the
  four disagreed with the code they claimed to describe.
- README: added a **Status** section (honest about how early this is), rewrote
  **How to** to the three commands that actually work, added a **Defining
  modules** paragraph saying plainly that UUIDs are hardcoded in two places and
  linking the open decision in plan 07, and added a **Repository layout** table.
  The conceptual material at the top was left alone -- it was the good part.
- Firmware header replaced with a real module doc: the characteristic table, the
  wiring (GPIO33 button / GPIO26 LED), and the flash command. The `//% FEATURES:`
  / `//% CHIPS:` markers are gone.
- `TODO(plan-05)` markers left on the hand-edited `-2` board suffix and on the
  read callback that should eventually carry module identity.

### The `Uuid` import did not work as planned

The plan said to add `use bleps::att::Uuid;`. Doing that produces an
`unused_imports` warning: the `gatt!` macro expands to that exact `use` inside the
same block, and the inner binding shadows the file-level one. So the import is
genuinely unused even though the name is used.

Resolved by naming the full path at the call site instead --
`bleps::att::Uuid::Uuid16(0x1809)` -- with a comment saying why. Same goal (the
origin is visible where you read it), no warning, and it no longer depends on the
macro's expansion at all.

### Deferred: renaming the `buttons` crate

Not done, on purpose. The crate is a button *and* an LED, so the name is wrong.
But plan 05 introduces a `role` concept (`modit-button-a`) that will want a
naming convention across firmware crates, and plan 09 may restructure the crates
entirely. Renaming now means renaming twice, and a rename touches the justfile,
the root manifest's `exclude`, the README and the flash instructions.

**Open decision — settle it as part of plan 05**, when the role vocabulary exists
and the right name is obvious.
