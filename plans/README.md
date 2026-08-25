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
| 04 | [A run that cannot fail](todo/04-fail-free-run.md) | fail-free run | L | todo |
| 05 | [Give each module a stable identity](todo/05-module-identity.md) | fail-free run | M | todo |
| 06 | [Make the firmware fail loudly](doing/06-firmware-hardening.md) | fail-free + errors | S | 🔨 code done, **needs a board** |
| 07 | [Make module definitions real](todo/07-typed-module-definitions.md) | easy to get into | M | todo |
| 08 | [The tutorial](todo/08-tutorial.md) | easy to get into | M | todo |
| 09 | [A seam to write scenarios against](todo/09-scenario-seam.md) | easy to get into | L | todo |

### Pick this up first

**Plan 06 is in `doing/` and needs hardware.** The code is written and linked, but
the panic-reboot path, the zero-length-write guard and the 50 ms debounce window
have never been exercised on a real board. Next time one is on the desk, work
through the verification list at the bottom of that plan and move it to `done/`.

Two open decisions are recorded rather than guessed:

- **Renaming the `buttons` crate** (it is a button *and* an LED) — deferred into
  plan 05, where the role vocabulary will make the right name obvious.
- **Whether the RON config workflow is wanted at all** — recorded in plan 07.

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

~~01, 02, 06 and 03.~~ Done — that was the 2026-08-25 pass. Next: **plan 04**
(a run that cannot fail). Start with the one-line `is_connected()` fix at the top
of it; everything else in that plan depends on disconnects being detectable.
