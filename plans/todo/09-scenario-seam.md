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

## Open questions

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

- [ ] Answer the open questions above, in the Notes section. Do this by writing two
      *different* scenarios on paper first (whack-a-mole and Simon Says) and seeing
      what they both need.
- [ ] Extract BLE plumbing from `brain/src/main.rs` into `crates/modit` (or a `lib.rs`
      in `brain` — a separate crate only if a second binary appears).
- [ ] Define the module handle API: `led().on()/.off()`, `wait_for_press(timeout)`,
      `is_connected()`.
- [ ] Reduce `brain/src/main.rs` to scenario code only.
- [ ] Add a fake/simulated backend so scenarios run with no hardware, driven by
      keyboard input. Feeds the tutorial's step 0 (plan 08).
- [ ] Write a second scenario (Simon Says) as a real test of the abstraction. If it
      needs runtime changes, the seam is in the wrong place — that is the point of
      writing it.
- [ ] Document the seam in `ARCHITECTURE.md`.

## Done when

- `brain/src/main.rs` contains game rules and no BLE types.
- A second, structurally different scenario exists and required no runtime changes.
- A scenario runs end to end against the simulated backend with no boards attached.

## Notes

_(answer the open questions here before writing any code)_
