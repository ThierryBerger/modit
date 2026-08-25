# Plans

Lightweight planning for a project that gets picked up and dropped for months at a time.

## Convention

| Folder  | Meaning |
| ------- | ------- |
| `todo/` | Planned, not started. Ordered by the `NN-` prefix. |
| `doing/`| Actively being worked on. **Keep at most one or two.** |
| `done/` | Finished. Kept for the "why did I do that" archaeology. |

Moving work = `git mv plans/todo/NN-foo.md plans/doing/`.

## Plan file shape

Every plan carries these sections so that future-you can re-enter it cold:

- **Why** — the pain this removes. One paragraph.
- **Current state** — what the code does today, with `file:line` references.
- **Target state** — what it should do instead.
- **Steps** — checkboxed, small enough to do in one sitting.
- **Done when** — observable acceptance criteria, not vibes.
- **Notes** — anything learned while doing it. Fill this in as you go.

## Reading order for a cold start

1. [`AUDIT.md`](AUDIT.md) — the state of the codebase as of the last full read-through.
2. The plans below, in order. The numbering encodes dependencies.

## Current plans

| # | Plan | Goal | Size | State |
| - | ---- | ---- | ---- | ----- |
| 01 | [One command to build everything](done/01-cargo-workspace.md) | easy to get into | S | ✅ done |
| 02 | [Delete the things that are not true](done/02-delete-the-lies.md) | easy to get into | S | ✅ done |
| 03 | [Error messages that tell you what to do](done/03-errors-and-logging.md) | good errors | M | ✅ done |
| 04 | [A run that cannot fail](doing/04-fail-free-run.md) | fail-free run | L | 🔨 code done, **needs boards** |
| 05 | [Give each module a stable identity](done/05-module-identity.md) | fail-free run | M | ✅ done |
| 06 | [Make the firmware fail loudly](doing/06-firmware-hardening.md) | fail-free + errors | S | 🔨 code done, **needs a board** |
| 07 | [Make module definitions real](done/07-typed-module-definitions.md) | easy to get into | M | ✅ done |
| 08 | [The tutorial](doing/08-tutorial.md) | easy to get into | M | 🔨 written, **never walked** |
| 09 | [A seam to write scenarios against](todo/09-scenario-seam.md) | easy to get into | L | designed, not built |

### Pick this up first

**Get two boards on the desk and work through [`CHECKME.md`](../CHECKME.md).**
Three plans are sitting in `doing/` waiting on hardware, and one of them (04)
rewrote the round from a poll loop into an event loop — the largest behavioural
change in the project, never run against a real module.

Then **plan 09**. Its design questions are answered (in the plan); only the
implementation is left. It absorbs the two items plan 04 could not do:
per-module recovery, and `--simulate` — the latter being the single most valuable
thing here for picking the project up again, since it makes scenarios testable
with no hardware at all.

Decisions settled in this pass, so they do not get relitigated:

- **No RON config workflow.** UUIDs are Rust constants in `shared::uuids`.
  Reasoning in plan 07.
- **`crates/buttons` renamed to `crates/module-button`**, matching the
  `modit-<role>-<id>` scheme.

### Why this order

01–02 are pure subtraction and take an afternoon: they make the repo honest before
anything is built on top of it. 03 comes next because every later plan is easier to
debug once failures explain themselves. 06 is small and independent — do it whenever
a board is on the desk, but before 04, since 04's chaos testing is unreadable while
a firmware panic is silent. 04 and 05 are entangled: reconnection needs identity, so
do 05's cheap version first. 07 is optional depending on one decision recorded inside
it. 08 is the actual deliverable and doubles as the acceptance test for 01–07 — but
write it last, because a tutorial written against the current code would document the
workarounds instead of removing them. 09 is the biggest and least urgent; its shape
depends on what 04 and 05 produce.

### If you only have one evening

Plug in two boards and run `CHECKME.md`. Everything else is written; what is
missing is confirmation that it works.
