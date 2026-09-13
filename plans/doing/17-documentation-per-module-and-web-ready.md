# 17 — Two registers of documentation, and a website to put them on

**Audit:** new (2026-09-12) · **Started:** 2026-09-12 · **Goal:** someone who is
not you can tell what this is for · **Size:** L · **Depends on nothing; blocks
nothing.** Written after the
`docs/` rewrite that produced [`WHY.md`](../../docs/WHY.md),
[`GETTING-STARTED.md`](../../docs/GETTING-STARTED.md) and
[`docs/hardware/`](../../docs/hardware/), so a good part of the first half is
already standing.

## Why

Everything written so far answers "how do I build this". Nothing answers "what
would I build it *for*", and nobody arrives at a repository with the second
question already settled.

Three different people need three different things, and today they all get the
same `README.md`:

| Who | Arrives asking | Gets today |
| --- | -------------- | ---------- |
| Someone deciding whether this is the right tool | "what can this run?" | a crate table and a BLE trade-off list |
| Someone designing a game with it | "what can I do with four buttons and a coin slot?" | four scenarios in a source file |
| Someone building a module | "what do I wire?" | this one is genuinely covered |

The third audience is served well. The first two are not served at all, and the
first one is the one that decides whether anybody reaches the other two.

There is also no website. Everything is markdown in a repository, which reaches
people who already found the repository. That is not the audience this is for:
the nomadic-game framing in [`WHY.md`](../../docs/WHY.md) describes sport
coaches, stand builders and festival organisers, and none of them read
`crates/`.

## Current state

**The non-technical layer exists in fragments, none of them addressed to a
non-developer.**
[`WHY.md`](../../docs/WHY.md) is the closest thing to a positioning page and is
already the right content — use cases, honest limits, what follows from the
framing. It is 96 lines of prose in `docs/`, linked from the middle of the
README, and it reads as a design rationale rather than as "here is what this
does".
[`README.md:35-48`](../../README.md) carries a "Target audience" section saying
the same thing more briefly.

**There is no page about composing a game from modules.**
The material for it is all in the code and nowhere in prose:

- `Modules` is the whole scenario-facing vocabulary — `set_output`,
  `next_press`, `next_coin`, `wait_for_press`, `next_event`, `reset_all`
  (`crates/brain/src/runtime.rs:60-206`).
- `Descriptor { role, inputs, outputs }` (`crates/shared/src/proto.rs:60-68`) is
  the machine-readable statement of what a module offers.
- `require` / `require_role` (`crates/brain/src/runtime.rs:347-380`) is where a
  scenario declares what it needs, and the doc comment there explains the
  mix-and-match failure mode better than any page currently does: waiting for
  coins from a button board does not error, it hangs.

A scenario creator has to read three source files to learn that a module is
"a role, N inputs, M outputs" and that scenarios are written against that, not
against a board.

**The technical per-module docs exist and are good, but live in one place while
the plan asked for another.**
[`docs/hardware/`](../../docs/hardware/) has a page per module, each with a bill
of materials, a net table, a generated schematic and the reasoning.
[`docs/HARDWARE.md:5-9`](../../docs/HARDWARE.md) indexes them with a status
column. The net tables are checked against the firmware by
`wiring_tables_match_firmware` ([`docs/hardware/README.md:28-41`](../../docs/hardware/README.md)),
so they cannot go stale.

`crates/module-button/` and `crates/module-coin/` contain **no README at all**.
Someone landing in the crate from GitHub's file browser, or from a `cargo`
listing, finds `main.rs` and nothing else.

**No module page says what the module is for.**
Every heading on all three pages is about construction: bill of materials, nets,
how it works, mistakes, checking it. `module-target` opens with "a plate you hit
with a ball" and is the only one that gestures at a use case at all. *(Three pages
at the time of writing. The `module-target` page was a design for an unbuilt
module and has since moved into [plan 15](../todo/15-impact-targets.md) — see the
cleanup note at the end.)*

**There is no site infrastructure of any kind.** No `book.toml`, no `mkdocs.yml`,
no `docs/index.html`, no Pages workflow — `.github/workflows/` contains only
`ci.yml`.

## Target state

Two registers, one source, per subject:

| | Non-technical | Technical |
| --- | --- | --- |
| The project | what you can run, who it is for, what it costs | `GETTING-STARTED` → `TUTORIAL` → `ARCHITECTURE` |
| Composing a game | what mixing modules lets you build | the `Modules` vocabulary and `require` |
| Each module | what use case it enables | BOM, net table, firmware |

And a website that publishes the whole thing, with the front door written for
somebody who has never opened a terminal.

Two properties matter more than the page list:

- **Nothing is said twice.** Two files describing the same pins is the failure
  this repository already refuses elsewhere — it is why the net table is the
  source of truth and why a test reads it. A module must not get a marketing
  page and a technical page that both list its capabilities.
- **Nothing claims more than the hardware does.**
  [`docs/HARDWARE.md:5-9`](../../docs/HARDWARE.md) already carries a per-module
  status column (`works` / `never met a board` / `design only`). A marketing page
  is precisely where that column goes missing, and plan 02 was about deleting the
  things that were not true. The status has to survive the trip to the front page.

## Decisions

Settled here so the steps below do not relitigate them.

**Technical module docs stay in `docs/hardware/`.** The plan's original note
offered "their own crate, for discoverability" or "at least their own folder in
docs". The folder wins, for reasons that are specific to this repository and not
general taste:

- `wiring_tables_match_firmware` reads those tables from a known path; moving
  them into two separate crates — each with its **own workspace and toolchain** —
  means a host test reaching into foreign trees.
- The schematics are generated by one script (`docs/hardware/schematics.py`,
  `just schematics`), and each `.svg` sits next to the page that embeds it.
- A module is a *harness*, not a crate. The impact target has a full design — bill
  of materials, nets, three schematics — and no crate at all, so crate-homed docs
  would have had nowhere to put it.

Each crate gets a **short README that points at its page** instead — enough for
someone who landed in the crate, with no second copy of the pin table.

**The site is mdBook + GitHub Pages, over a staged copy of the markdown.**
Timeboxed and settled, as the step below asked. mdBook renders the `docs/` and
`plans/` markdown as-is; the whole commitment is `book.toml`, two scripts in
`docs/site/`, and one workflow, and abandoning it deletes exactly those.

The one wrinkle worth recording, because it is not obvious and cost the most time
here: mdBook needs a single `src` directory containing a `SUMMARY.md`, and pointing
it at `docs/` breaks every link that leaves `docs/` (`../plans/…`, `../CHECKME.md`)
while pointing it at the repository root would publish `crates/*/target/`. So
[`docs/site/stage.py`](../../docs/site/stage.py) copies `docs/`, `plans/` and
`CHECKME.md` into `target/site/src` **at their repository paths**, which makes
every existing cross-link work untouched, and rewrites only the links that leave
that set — into source code — to GitHub blob URLs.

That is a generated copy, not a second source: no page exists twice, and the
copy is disposable. It is the same trade the schematics already make.

Two mdBook behaviours are handled there rather than worked around in the prose,
both of which produced links that worked on GitHub and 404'd on the site:

- a chapter called `README.md` is rendered as its directory's `index.html`, but
  links *to* `README.md` are rewritten to a `README.html` that is never written;
- the first chapter is also copied to the site root **with its own relative links
  intact**, so a landing page one directory down produces a root page broken by
  exactly one level. The landing page is therefore staged *at* the root.

`docs/site/check_links.py` reads the rendered HTML and fails on any internal link
with no page behind it. It runs in `just site` and in CI, and it is the reason
those two behaviours are known rather than live on the published site.

**The repository `README.md` is not on the site.** It is the *repository's* front
door — crate table, build commands, layout — and `docs/index.md` is the site's,
written for someone who has not opened a terminal. Two front doors in one
navigation is the duplication this plan exists to avoid; links to it leave the
site like any other source link.

**One page per module, non-technical section first.** Not two files. The "what
this is for" section goes at the top of the existing `docs/hardware/<module>.md`,
above the bill of materials. Splitting it doubles the number of places a new
capability has to be described, and the second one always rots.

## Steps

The first three are writing and need no decision about the site. Do them first —
they are what the site would publish anyway, and they are useful with no site at
all.

- [x] **Open each module page with what it is for.** A short section above
      "Bill of materials" on all three pages in `docs/hardware/`: what the module
      senses or does, what kind of game beat it produces, one concrete example,
      and its status. Written for someone who will never flash it.
      *Done: "What it is for" on button and coin, "What it **would** be for" on
      target, which does not exist.*
- [x] **Write `docs/COMPOSING.md`** — the scenario-creator page that does not
      exist. What a module is from a game's point of view (a role, N inputs,
      M outputs), what the `Modules` vocabulary offers, what combinations the
      existing four scenarios are each an example of, and what `require_role`
      catches. Aimed at someone writing their first scenario, not at someone
      reading `runtime.rs`.
- [x] **Add a capability table to that page**, one row per module: role, inputs,
      outputs, what each channel means. This is the mix-and-match answer in one
      screen. *Done, with no status column: the status lives in
      `docs/HARDWARE.md` and the row links to it rather than copying it.*
- [x] **Check that table against the firmware**, the same way the net tables are
      checked. Each firmware builds a `Descriptor`; a host test should read the
      markdown table and fail when they disagree. Without this the table is a
      comment that used to be true, and this repository has a stated position on
      those.

      *Done as `capability_table_matches_the_modules` in
      [`crates/brain/src/main.rs`](../../crates/brain/src/main.rs), and it reaches
      the firmware by a longer route than the plan assumed.* `module-button` has
      no `Descriptor` yet, so the numbers are checked against that module's **net
      table**, which `wiring_tables_match_firmware` already pins to the firmware's
      pins — the chain is capability table → net table → `peripherals.GPIOnn`.
      Where a firmware *does* declare a `Descriptor` (coin) the role is checked
      too, which no net table knows about. The `None` entry for `module-button`
      asserts that absence, so the day plan 11 step 2 gives it a descriptor the
      test fails and demands the stronger check. The reverse direction is covered
      by `Role::ALL`: a role with no row in the table fails.
- [x] **A README per firmware crate.** Five lines: what the module is, a link to
      its hardware page, the flash command. No pin tables — the page owns those.
      *Done for both. `shared` and `brain` got none: `docs/ARCHITECTURE.md` is
      their page, and a README repeating it is the thing this plan is against.*
- [x] **Decide the site toolchain, cheaply.** The constraint is that the source
      of truth stays the markdown already in `docs/`, because a site with its own
      copy of the content is the duplication this plan is trying to avoid.
      Recommendation: **mdBook + GitHub Pages** — it renders `docs/` as-is, the
      build is one CI step, and it costs nothing to abandon. A hand-written
      landing page over a generated docs tree is the other credible option and
      can be layered on later. **Timebox this and record the answer here.**
- [x] **Publish the docs tree** at whatever the previous step chose, from CI, on
      push to `main`. Plain and unstyled is fine; this step is about the pipeline
      existing, not about how it looks. *Done:
      [`.github/workflows/docs.yml`](../../.github/workflows/docs.yml), building on
      pull requests and publishing from `main`. **Never executed** — there is no
      push yet, and Pages has to be switched to "GitHub Actions" in the repository
      settings by hand before the first deploy can work.*
- [x] **Write the landing page.** The one page for somebody who has not decided
      yet: what a module is, what a game made of them looks like, the use cases
      from `WHY.md`, the honest limits, and one link to "make your own". This is
      the page the whole plan is for, and it is last because it is the easiest
      one to write badly before the others exist. *Done as
      [`docs/index.md`](../../docs/index.md) — short, and mostly pointing
      elsewhere, because `WHY.md` already holds the use cases and the limits and a
      second copy of either would rot.*
- [ ] **Put a picture or a recording on it.** A terminal transcript is not a
      demonstration of a physical game. Even a phone video of two boards and a
      button is worth more than the rest of the page. **This needs the boards
      that `CHECKME.md` needs**, so it is the one step that cannot be done at a
      desk. *Left open, and added to `CHECKME.md` as item 10 so it is picked up
      the next time boards are on the desk rather than remembered.*

## Done when

- [x] Each module page answers "what would I use this for?" before it answers
      "what do I solder?", and states its status in the same breath.
- [x] `docs/COMPOSING.md` is enough to design a scenario on paper — pick modules,
      know what each offers, know what the brain will refuse — without opening
      `runtime.rs`.
- [x] The capability table is verified by a test, so adding a module type with a
      new `Descriptor` and forgetting the docs fails CI.
- [x] Every firmware crate has a README that links onward, and no crate README
      restates a pin.
- [ ] Pushing to `main` publishes the documentation, and the URL is in the
      repository description and at the top of `README.md`.
      *Half done: the workflow exists and the URL
      (<https://vrixyz.github.io/modit/>) is the first line of `README.md`. What
      is left is not writing — enable Pages with the "GitHub Actions" source, push,
      and paste the URL into the repository description. Until that is done the
      link in `README.md` is a 404, which is the one claim in this pass that is
      ahead of reality.*
- [x] The landing page never claims a module works when
      [`docs/HARDWARE.md`](../../docs/HARDWARE.md) says it has not met a board.
- [ ] Someone who is not you can read the landing page and correctly say what
      this project does and whether it suits them. **This one needs an actual
      person; it is the only real acceptance test on the page.**

## Notes

- The original version of this plan asked for "a non-technical page per module"
  *and* "a technical doc per module". Two files per module was the instinct, and
  it is wrong for the same reason the schematics are generated rather than drawn:
  the second copy is the one that goes stale. One page, two registers, ordered
  non-technical first.
- The largest risk here was writing a marketing page for a project whose most
  interesting modules do not exist yet. `module-coin` has never met a board and
  cannot run over BLE at all; the impact target has no firmware. A page showing
  all three as products would have been a lie with a schematic attached, and the
  cleanup below resolved it the blunt way: the target is not on the site, because
  it does not exist.
- `WHY.md` is already most of the landing page's text and was written for this
  before there was a page to put it on. The landing page is largely an edit of it
  for a reader who has not opened a terminal, not a new document.
- Doing the writing steps first is deliberate. A site with no content is a
  pipeline; content with no site is still documentation.

### Learned while doing it (2026-09-12)

- **The capability table was the cheap part; verifying it was the design work.**
  The plan assumed each firmware builds a `Descriptor` to compare against, and one
  of the two does not. Going through the *net table* instead turned out better
  than waiting for plan 11: the net table is already machine-checked against the
  pins, so the chain reaches the firmware anyway, and it also covers channel
  counts for a module whose firmware has not been written in protocol terms yet.
- **"Nothing is said twice" bit hardest on the status.** Four files already carry
  some version of "the coin module has never met a board" (`README.md`,
  `docs/HARDWARE.md`, the module page, `docs/GETTING-STARTED.md`). The new pages
  add no fifth: `COMPOSING.md` has no status column and the landing page names the
  status table rather than reproducing it. This is still the most likely thing in
  the tree to drift, and `docs/HARDWARE.md` is the one to edit when it does.
- **A link check earned its place immediately.** Both mdBook link traps recorded
  above were found by `check_links.py` on the first build, not by reading. Neither
  is visible in the markdown — the links are correct on GitHub and wrong on the
  site — so no amount of proofreading would have caught them.
- The only steps left need something a desk does not have: boards for the
  photograph ([`CHECKME.md`](../CHECKME.md) item 9), a GitHub setting for the
  deploy, and a person who is not the author for the acceptance test.

### The cleanup pass that followed (2026-09-12)

Publishing the tree made its stale parts obvious, so a second pass applied one
rule: **documentation describes what exists, and planning lives in `plans/`.**

- **`plans/AUDIT.md` deleted.** Every one of its 28 findings was fixed or void —
  finding 21 assumed the RON workflow plan 07 then decided against. It described
  a `buttons` crate and a game loop inside `main` that no longer exist, and git
  keeps it.
- **`CHECKME.md` moved to `plans/CHECKME.md`**, because it is planned work and was
  the largest source of plan references outside `plans/`. Its settled-decisions
  section went with the move: those decisions are recorded in `plans/README.md`,
  and the one open question in it — whether a 30 s reconnect backoff is too slow
  for a real game — went into [plan 04](04-fail-free-run.md).
- **The impact-target design folded into [plan 15](../todo/15-impact-targets.md)**,
  with its figures now generated into `plans/figures/`. A `docs/hardware/` page for
  a module with no firmware was the biggest single "what we do not do" in the tree,
  and its rows left `docs/HARDWARE.md` and the capability table — which deleted
  `DESIGN_ONLY` from the test, so every documented module is now checked against a
  real firmware.
- **Every `plan NN` reference outside `plans/` is gone** — from source comments, a
  user-facing error message, and eight pages. Each became a statement of the
  present instead: "the BLE link cannot drive a coin acceptor: it still speaks the
  button firmware's wire format" says the same thing to someone who will never
  open `plans/`.
- **`docs/ARCHITECTURE.md` was the stalest file in the repository** and is the one
  this plan nearly missed: three crates where there are four, two workspaces where
  there are three, three scenarios where there are four, a rough-edges section
  describing a protocol that now exists, and a dangling "Those two are the same
  seam" whose other half had been deleted. A page nobody links to from the front
  door rots quietly; the site is what made it visible.
- **`.env.example` deleted.** Nothing has read `.env` since `dotenvy` was removed;
  the file documented a variable that `just flash a` already sets.

One thing deliberately left: the `**Audit:** 5, 26` headers on the older plans
still name findings from the deleted audit. They are provenance for why a plan
exists, they are inside `plans/`, and renumbering them would be churn with no
reader.
