# 09 — A seam to write scenarios against

**Audit:** 22 · **Goal:** straightforward to get into · **Size:** large

## Why

The README promises a "moddable brain": *"a single brain as a client, higher level
code a user tweaks to its needs"*. There is currently no place to put that code.
The whack-a-mole game is interleaved with BLE plumbing inside `main()`
(`brain/src/main.rs:54-160`) — spawn logic, mutexes, an `AtomicBool`, and the game
rules all in one function. Writing a second game means editing `main` and
understanding btleplug.

This is the largest plan and the least urgent. Do it after the run is reliable
(plan 04) and modules have identity (plan 05) — a seam designed before those exist
will be the wrong shape.

## Current state

`brain/src/main.rs` `main()` does, in one function: adapter setup, the acquire
retry loop, LED reset, RNG, per-module task spawning, disconnect detection, the
game rules, and the re-arm scheduling. `ButtonLed` / `ButtonDetails`
(`main.rs:17-27`) are the only concession to structure.

## Target state

Two layers with a named boundary:

- **Runtime** (a `modit` library crate) — owns adapters, scanning, binding,
  reconnection, and the notification streams. Exposes modules as handles with
  methods, and delivers events.
- **Scenario** (the user's `main.rs`) — plain async Rust against those handles. No
  btleplug types in its signatures.

Sketch, to be refined against what plans 04 and 05 actually produce:

```rust
// what a user writes
async fn whack_a_mole(modules: &Modules) -> Result<()> {
    let buttons: Vec<ButtonLed> = modules.all_of_role("button")?;
    loop {
        for b in &buttons { b.led().off().await?; }
        let target = buttons.choose(&mut rng);
        sleep(random_delay()).await;
        target.led().on().await?;
        match target.wait_for_press(Duration::from_secs(5)).await {
            Ok(()) => info!("hit"),
            Err(Timeout) => info!("too slow"),
        }
    }
}
```

Key properties:

- The scenario never sees a `Peripheral`, a `Characteristic`, or a `JoinHandle`.
- A module that drops out mid-scenario surfaces as a typed error the scenario can
  ignore or handle — it does not abort the process (plan 04).
- The scenario is testable against a fake `Modules` with no radio.

> **Read [plan 11](11-message-protocol.md) first.** It replaces UUID-as-protocol
> with typed messages in `shared`, and its `Link` trait is this plan's seam seen
> one layer lower. The two are best done together — 11 changes the primitive
> below from `next_press` to `next_event`. Doing 09 alone means designing the
> scenario API around press-shaped calls and then generalising it.

## Design work done 2026-08-25 (not implemented)

The plan says to answer the open questions by writing two different scenarios on
paper first. That was done. Results below; the implementation is still to do.

### The two scenarios, on paper

**Whack-a-mole** (what exists): light one module, wait for *that* module's press,
light another. Wrong presses are logged and ignored. All modules are watched
concurrently.

**Simon Says**: light a growing sequence with pauses, then wait for the player to
press the same modules *in order*. A wrong press ends the round.

### What that comparison revealed

**The primitive is the wrong shape in the obvious design.** `wait_for_press()` on
a single module handle serves whack-a-mole and cannot serve Simon Says, which needs
"the next press from *any* module, tell me which". Building the single-module
version first and generalising later means rewriting every scenario.

So the primitive is:

```rust
// Blocks until any module in the set is pressed, or the timeout expires.
async fn next_press(&self, timeout: Duration) -> Result<ModuleId, WaitError>;
```

and `module.wait_for_press(timeout)` becomes a thin filter over it. Both scenarios
fall out of that; neither falls out of the other order.

**Updated 2026-08-27, after plan 11:** the same argument applies one level up.
With a message protocol the primitive is

```rust
async fn next_event(&self, timeout: Duration) -> Result<(ModuleId, Event), WaitError>;
```

and `next_press` becomes a filter over *that*. A press is one `Event` variant; a
sensor reading is another. Building `next_press` as the primitive would have to be
widened the moment a module type reports anything other than a press — which is
the same mistake as building `wait_for_press`, one layer out.

**Both scenarios need `Modules` as a set, not a Vec of handles.** Whack-a-mole
picks a random one; Simon Says builds a sequence. Both want "all modules with role
`button`" and stable ids, not indices.

**Neither needs a lifecycle hook.** No setup/teardown/tick beyond what plain async
code expresses.

## Answers to the open questions

| Question | Answer | Why |
| -------- | ------ | --- |
| What happens to a scenario when a module is lost? | Return `Err(WaitError::ModuleLost(id))` from the awaiting call. Do not suspend, do not abort the process. | The two scenarios want different things — whack-a-mole can carry on with the rest, Simon Says must end the round. Only the scenario knows. A helper for "wait until it comes back" covers the third case. |
| One scenario or several concurrent? | One, for now. `run(scenario)` takes the whole module set. | Several needs disjoint ownership, which is a real design in itself. Nothing today wants it, and the single-scenario API is a strict subset. |
| Does the runtime own the event loop, or the scenario? | The runtime. `modit::run(modules, scenario).await`. | The runtime already has to own acquisition, reconnection and the notification streams. Handing the loop to the scenario means handing it all of that too. |
| Where does the transport go? | Behind a `Link` trait *below* the runtime — see [plan 11](11-message-protocol.md). | The runtime deals in `(ModuleId, Event)`; whether those arrive over BLE, an in-process channel (`--simulate`), or ESP-NOW later is not its concern. |
| Trait, or an async fn taking `&Modules`? | An async fn. | Neither scenario needs a lifecycle hook, so a trait would be ceremony. It can become one later without changing call sites. |

## Original open questions

- **What happens to a scenario when a module is lost?** Await transparently until it
  returns? Return `Err(ModuleLost)`? Suspend the whole scenario? Different games want
  different answers; pick a default and make it overridable.
- **One scenario or several concurrent ones?** An escape game plausibly wants
  independent puzzles running at once, each owning a disjoint set of modules.
- **Does the runtime own the event loop, or does the scenario?** A `run(scenario)`
  entry point is simpler; a scenario owning its own loop is more flexible.
- **Is a trait right, or is an async fn taking `&Modules` enough?** Start with the
  function. A trait can come later if scenarios need lifecycle hooks.

## Steps

- [x] Answer the open questions above. **Done 2026-08-25 — see the section at the
      top. Implementation not started.**
- [x] Build `next_press(set, timeout)` as the primitive **first**, then express
      `wait_for_press(module, timeout)` in terms of it. Doing it the other way
      round means rewriting every scenario.
- [x] Extract BLE plumbing from `brain/src/main.rs` into `crates/modit` (or a `lib.rs`
      in `brain` — a separate crate only if a second binary appears).
- [x] Define the module handle API: `led().on()/.off()`, `wait_for_press(timeout)`,
      `is_connected()`.
- [x] Reduce `brain/src/main.rs` to scenario code only.
- [x] Add a fake/simulated backend so scenarios run with no hardware, driven by
      keyboard input. Feeds the tutorial's step 0 (plan 08). **Done: `just simulate`.**
- [x] Write a second scenario (Simon Says) as a real test of the abstraction. If it
      needs runtime changes, the seam is in the wrong place — that is the point of
      writing it.
- [x] Document the seam in `ARCHITECTURE.md`.

## Done when

- `brain/src/main.rs` contains game rules and no BLE types.
- A second, structurally different scenario exists and required no runtime changes.
- A scenario runs end to end against the simulated backend with no boards attached.

## Notes

### Implemented 2026-08-28

Built, and — unusually for this project — **actually verified**, because the
plan's own deliverable is a test harness.

#### Shape

```
scenarios.rs        whack_a_mole, simon_says -- plain async fns over &Modules
runtime.rs          Modules handle, run(), WaitError, require()
link/mod.rs         Link / LinkTx / LinkRx traits, ModuleId, ModuleEvent
link/ble.rs         thin wrapper over the existing ble/ code
link/sim.rs         in-process channels: --simulate, and the test harness
ble/                UNCHANGED
```

`Modules` is deliberately **not** generic over the transport. One task owns the
link and communicates over channels, so a scenario signature never mentions BLE,
`Link`, or a type parameter. `Link` splits into `Tx` (cloneable, shared) and `Rx`
(owned, mutable) so both can sit in one `select!` without borrowing the same
value.

#### Simon Says needed no runtime changes

That was the plan's stated test of whether the seam is in the right place, and it
passed: `simon_says` is structurally unlike whack-a-mole — it waits on *any*
module and checks which arrived, and a wrong press ends the round rather than
being ignored — and it compiled against the API as designed.

#### The bug that a flaky test found

Worth recording, because the lesson is not about the bug.

`simon_says` originally filtered events by hand:

```rust
for (step, expected) in sequence.iter().enumerate() {
    match modules.next_event(PATIENCE).await {
        Ok((who, Event::Pressed{..})) if &who == expected => { /* correct */ }
        Ok((who, Event::Pressed{..})) => { /* wrong, end round */ }
        Ok((from, other)) => { debug!("ignoring {other:?}"); }   // <-- bug
        ...
    }
}
```

The ignore arm **consumes the loop iteration**. Every module sends `Hello` on
connect, so a queued `Hello` silently satisfied a step without anybody pressing
anything. The test failed roughly 40% of the time depending on whether the
Hellos were drained before the loop started.

The fix was not an inner loop in the scenario. It was to add the filtered
primitive the design section above already called for:

```rust
next_event(timeout)  -> (ModuleId, Event)   // the primitive
next_press(timeout)  -> (ModuleId, u8)      // filter: any module, presses only
wait_for_press(id, timeout) -> u8           // filter: one module
```

`wait_for_press` is now written in terms of `next_press`, which is written in
terms of `next_event`. Hand-filtering is still possible but no longer the obvious
path, and the doc comment on `next_press` says why.

**This is the argument for `--simulate` in one story.** The bug is invisible by
inspection, would have shown up on hardware as "Simon Says sometimes skips a
step", and was diagnosed in minutes because the scenario runs in a test.

#### Making the tests non-flaky

Two separate fixes, in order:

1. **Virtual time.** `#[tokio::test(start_paused = true)]` (needs tokio's
   `test-util`, added as a dev-dependency) makes `tokio::time` virtual, so the
   scenario's real sleeps cost nothing and the tests do not depend on wall clock.
   Runtime went 2.51s → 0.01s.
2. **Assert on history, not on transient state.** One test polled for the
   game-over blink — a 150 ms window. Whether a poll catches a blink depends on
   scheduling, which is a flaky test by construction. `SimHandle` now records
   every command, and tests assert on that log instead.

Neither of those fixed the real bug; they made it *visible* rather than
intermittent. Verified with **60 consecutive full test runs, zero failures**,
after 18/40 failures before.

#### Verified

- 26 tests, 17 of them in `brain`, including 8 scenario tests.
- Mutation check: breaking the "is it the right module" logic fails 3 tests.
- `just simulate` plays a full game from the keyboard, including `-a` to drop a
  module and watch the runtime recover.
- `just simulate --scenario simon` plays Simon Says.
- Clippy clean under `-D warnings`.

#### Still not done

**Per-module demotion and background re-acquisition.** `WaitError::ModuleLost`
exists and scenarios choose how to react — whack-a-mole carries on if the lost
module was not the lit one, Simon Says always ends the round — which is the
scenario-facing half. The runtime half is not built: a lost module is not
re-acquired in the background, so a scenario that keeps playing may later try to
light a module that is gone, ending the round. Recovery is still coarse.

That wants a supervisor owning module handles that can be temporarily absent.
It is the last piece of this plan.

### Why implementation was deliberately deferred (2026-08-25)

Plans 01–08 landed in one pass. Plan 04 rewrote the round from a poll loop into an
event loop holding notification streams — the largest behavioural change in the
project — and **it has never run against hardware.**

Stacking a large architectural refactor on top of that would mean that when
something misbehaves on a real board, there is no way to tell whether the cause is
the event loop or the seam. The event loop should be confirmed working first; then
this plan starts from a known-good base.

That is the only reason. The design above is settled and the work is ready to
start.

### What this plan should absorb when it runs

Two items were moved here from plan 04 because they need this seam:

1. **Per-module demotion and re-acquisition** (`ModuleState { Bound, Lost }`).
   Recovery today is coarse: any module failing ends the round and everything is
   re-acquired. Nothing panics and the run continues, but one wobbly board
   interrupts the whole game. The `WaitError::ModuleLost` answer above is the
   scenario-facing half of this.

2. **`--simulate`**, a backend with no radio, driven by keyboard input. This is
   the single highest-value item in the whole backlog for picking the project back
   up — it makes scenarios testable on a laptop with no boards in the bag, and it
   gives the tutorial a step 0. It only becomes possible once the runtime is
   behind a trait.

   With [plan 11](11-message-protocol.md) done, this is close to free: `--simulate`
   is a `Link` impl over an in-process channel carrying the same `Command`/`Event`
   enums the radio would carry.
