# 01 — One command to build everything

**Audit:** 17, 23, 28 · **Goal:** straightforward to get into · **Size:** small

## Why

Right now `cargo build` at the repo root does nothing at all. There are three
crates, three `Cargo.lock` files, three byte-identical `.gitignore`s and three
`target/` directories. Coming back to this project, the first thing you do is
guess which directory to `cd` into — and there is nothing that tells you the
firmware needs a different toolchain until a build fails with a linker error.

## Current state

- No root `Cargo.toml`.
- `crates/{brain,shared,buttons}/` each carry their own lockfile and `.gitignore`.
- `crates/buttons/rust-toolchain.toml` pins `channel = "esp"`; `crates/buttons/.cargo/config.toml`
  pins `target = "xtensa-esp32-none-elf"` and `build-std`.
- `ron` is declared in `brain/Cargo.toml` and never used.
  `dotenvy_macro` is declared in `buttons/Cargo.toml` and never used.

## Target state

Two workspaces, deliberately, with the reason written down:

- **Root workspace** — `shared` + `brain`. Host toolchain. `cargo check` at the
  repo root checks both.
- **`crates/buttons`** — stays a standalone workspace (`[workspace]` empty table in
  its `Cargo.toml` so cargo does not try to absorb it). It has its own toolchain
  and target and cannot join the root.

A `justfile` (or `Makefile.toml` / plain `make` — pick one and commit to it) hides
the split behind named commands:

```
just check      # both workspaces
just brain      # run the host binary with sensible RUST_LOG
just flash      # espflash the firmware, with --monitor
just fmt        # both workspaces
```

## Steps

- [x] Add root `Cargo.toml` with `[workspace] members = ["crates/shared", "crates/brain"]`
      and `resolver = "3"`.
- [x] Add `[workspace] exclude = ["crates/buttons"]` and an empty `[workspace]` table
      in `crates/buttons/Cargo.toml` so the exclusion is explicit from both sides.
- [x] Delete `crates/brain/Cargo.lock` and `crates/shared/Cargo.lock`; keep a single
      root lock. Keep `crates/buttons/Cargo.lock` — it is a separate workspace.
- [x] Collapse the three `.gitignore`s into the root one.
- [x] Move shared metadata (`edition`, `version`) to `[workspace.package]`.
- [x] Remove the unused `ron` and `dotenvy_macro` dependencies. If `.env` was meant
      to configure something, write it down in plan 07 instead of leaving the dep.
      Also drop the empty `[env]` block in `crates/buttons/.cargo/config.toml`.
- [x] Add a `justfile` with the commands above.
- [x] Add a one-paragraph comment at the top of the root `Cargo.toml` explaining
      *why* `buttons` is excluded. This is the note that saves the next re-entry.

## Done when

- `cargo check` at the repo root checks `shared` and `brain` and nothing else.
- `just check` checks all three crates and exits non-zero if any fails.
- `git ls-files '*Cargo.lock'` returns exactly two paths.
- No crate declares a dependency it does not use.

## Notes

Done 2026-08-25.

- Root `Cargo.toml` holds `shared` + `brain`, `resolver = "3"`, and a
  `[workspace.package]` block so both members inherit `version` and `edition`.
  `shared` and `anyhow` are `[workspace.dependencies]`.
- `crates/buttons/Cargo.toml` now carries an empty `[workspace]` table, making it
  a workspace root in its own right. Both sides of the split are explicit: the
  root `exclude`s it, and it declares itself. The reasoning is written as a
  comment at the top of the root manifest -- that comment is the point of the
  whole plan.
- Lockfiles: two, as intended. The root one is `crates/brain/Cargo.lock` promoted
  (git recorded it as a rename), `crates/shared/Cargo.lock` deleted.
- **Gotcha worth remembering:** deleting the per-crate `.gitignore`s exposed the
  stale `crates/{brain,shared}/target/` directories, which the root ignore did not
  match. Fixed by using a bare `target/` pattern (matches at any depth) rather
  than `/target` + `/crates/buttons/target`, and deleting the now-obsolete dirs --
  the host crates build into the root `target/` now.
- `ron` (brain) and `dotenvy_macro` (buttons) removed as unused, along with a
  commented-out `#ron` line and the empty `[env]` block in the firmware's
  `.cargo/config.toml`. Plan 05 may reintroduce `dotenvy_macro` for `MODIT_ID`.
- `justfile` covers `check`, `check-host`, `check-firmware`, `brain`, `flash`,
  `monitor`, `fmt`, `fmt-check`, `clippy`, `test`, `ci` and `setup`.
  `just setup` prints the `source ~/export-esp.sh` reminder, which is the step
  that always gets forgotten.

Verified: `just check` builds both workspaces, `just clippy` passes with
`-D warnings` on both, `git ls-files '*Cargo.lock'` returns exactly two paths.
