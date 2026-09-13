# Plans

Lightweight planning for a project that gets picked up and dropped for months at a time.

## Convention

| Folder  | Meaning |
| ------- | ------- |
| `todo/` | Planned, not started. Ordered by the `NN-` prefix. |
| `doing/`| Started. **Keep at most one or two actually in progress** — several sit here code-complete, waiting on a board. |
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

1. [`CHECKME.md`](CHECKME.md) — what is written but has never been watched work.
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
| 09 | [A seam to write scenarios against](doing/09-scenario-seam.md) | easy to get into | L | 🔨 built + tested; per-module recovery left |
| 10 | [Earn the battery life BLE was chosen for](todo/10-power-and-battery.md) | modules run on batteries | M | todo — **starts with a measurement** |
| 11 | [Put the protocol in `shared`, not in the UUIDs](doing/11-message-protocol.md) | scales past one module type | L | 🔨 step 1 done; steps 2-4 need a board |
| 12 | [A coin acceptor module: protocol, simulator, Uno](done/12-coin-module-uno.md) | a second module type | L | ✅ done — an Uno counts real coins |
| 12 | [A coin acceptor module: the ESP32, and coins over BLE](doing/12-coin-module-esp32.md) | a second module type | M | 🔨 firmware written, **needs the acceptor on an ESP32**; BLE waits on 11 |
| 13 | [A brainless mode: modules pass a token](todo/13-brainless-token-mesh.md) | a game with no laptop | L | todo — **isolated experiment**, starts with a radio spike |
| 14 | [Catch every input, not just the slow ones](doing/14-edge-latched-inputs.md) | no input is silently dropped | M | 🔨 steps 1-3b done + tested, **step 4 needs a board** |
| 15 | [Targets you hit with a ball](todo/15-impact-targets.md) | a game played with a racket | L | todo — **starts with one piezo and a plank** |
| 16 | [Survive the Bluetooth stack dying](todo/16-surviving-a-dead-adapter.md) | fail-free run | M | todo — **from a real incident**, three boards on the desk |
| 17 | [Two registers of docs, and a website](doing/17-documentation-per-module-and-web-ready.md) | someone else can tell what this is for | L | 🔨 pages + site written; **needs Pages enabled**, and a photo |

### Pick this up first

**Get two boards on the desk and work through [`CHECKME.md`](CHECKME.md).**
Most of `doing/` is code that is written, tested on the host, and waiting on a
board — including 04, which rewrote the round from a poll loop into an event loop.
That is the largest behavioural change in the project and it has never run against
a real module.

**`just simulate` runs the whole game with no hardware** — that is the fastest way
back into this project, and it is also the test harness. Try
`just simulate --scenario arcade` and type `$` to post a coin.

What is left, in order:

1. **Hardware verification** (`CHECKME.md`) — three plans wait on it.
2. **Plan 11 steps 2–4**: move the firmware onto the real `Command`/`Event`
   messages. Step 1 (the types) is done and tested; the firmware half needs a
   board, so it waits for 1. **[Plan 12](doing/12-coin-module-esp32.md) step 5 is
   blocked on exactly this** — the coin module runs under `--simulate` and
   cannot run over BLE until the link stops being typed to buttons.
3. **Plan 09's last piece**: per-module demotion and background re-acquisition.
   `WaitError::ModuleLost` exists and scenarios already choose how to react; the
   runtime just does not bring a lost module back yet.
4. **Plan 10**: one measurement, which may close it.
5. **Plan 13** is deliberately off to one side: a brainless mode where modules
   pass a token between themselves over BLE advertisements. It touches nothing
   else in the tree, so it can be picked up whenever the centralised path is
   blocked on hardware -- though its own first step needs two boards.
6. **Plan 16** is the one with a real failure behind it rather than a
   read-through: a laptop slept mid-game, CoreBluetooth died, and the brain kept
   scoring rounds against boards it could no longer reach. It closes plan 04's
   chaos row 5 and finally does plan 09's per-module recovery. Its first two
   steps are a log-format change and a deliberate reproduction, so it starts
   cheap.
7. **Plans 14 and 15** are the ball-game branch. 14 is a real bug fix worth
   doing on its own -- `module-button` samples its pin from the BLE work loop, so
   any input shorter than a finger press can be dropped silently -- and its first
   step is host-testable. 15 depends on it and starts with a €5 experiment on a
   plank, not with building modules.
8. **Plan 17** is mostly done: every module page now opens with what it is for,
   [`docs/COMPOSING.md`](../docs/COMPOSING.md) is the page for designing a game
   out of modules (with a capability table a test keeps honest), both firmware
   crates have a README, and `just site` builds an mdBook site out of the existing
   markdown. What is left is not writing: **switch Pages to "GitHub Actions" in
   the repository settings** so the workflow can deploy, then photograph two
   boards for the landing page (`CHECKME.md` item 10).

Plan 10 is independent of the rest and needs only one board and a USB power
meter. Its first step may conclude "the current firmware already meets the
requirement", which is a real outcome — do not skip straight to optimising.

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
