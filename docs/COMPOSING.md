# Composing a game out of modules

For someone designing a game, before writing any code. It answers "what can I
build out of the modules that exist, and what will the brain refuse?" -- on
paper, without reading `runtime.rs`.

Wiring is in [Hardware](HARDWARE.md), getting something running is in
[Getting started](GETTING-STARTED.md), and how the layers fit is in
[Architecture](ARCHITECTURE.md).

## A module is not a board

To a game, a module is three facts:

- a **role** -- what kind of thing it is (`Button`, `Coin`),
- a number of **inputs**, addressed `0..inputs`,
- a number of **outputs**, addressed `0..outputs`.

That is all. It is one struct,
[`Descriptor`](../crates/shared/src/proto.rs), and every module sends it
unprompted when it connects. A scenario says "turn on output 0 of `a`" and "wait
for a press on `b`"; which pin that is, and whether `b` is on a breadboard or in
a simulator, is not a question a scenario can ask.

Three things follow, and all three matter when designing:

- **Channels are numbers, not names.** There is no `led` or `button` in a
  scenario -- output 0, input 0. The names live on the module's hardware page,
  where the wiring is.
- **A module is a box, not a component.** Three buttons and two LEDs in one
  enclosure is *one* module with `inputs: 3, outputs: 2`, not three modules. It
  connects once and counts as one of your BLE connections.
- **The same game runs with no hardware.** `just simulate` presents modules with
  the same descriptors over the same vocabulary, so a game can be designed and
  played on a train and then met with boards later.

## What each module offers

| Module | Role | Inputs | Outputs | What the channels are |
| ------ | ---- | ------ | ------- | --------------------- |
| [module-button](hardware/module-button.md) | `Button` | 1 | 1 | **in 0** the button -- `Pressed` on the rising edge, `Released` on the falling one. **out 0** the LED. |
| [module-coin](hardware/module-coin.md) | `Coin` | 1 | 0 | **in 0** the acceptor's pulse line -- one `Coin { pulses }` per accepted coin. |

Whether a module has met real hardware is the status column in
[Hardware](HARDWARE.md); it is not repeated here. `module-coin` plays under
`--simulate` but cannot be driven over BLE, because the radio link still speaks
the button firmware's own wire format.

The counts in that table are checked by a test against each module's net table --
which is itself checked against the firmware's pins -- so a module that grows a
channel cannot leave this page behind. See `capability_table_matches_the_modules`
in [`crates/brain/src/main.rs`](../crates/brain/src/main.rs).

## The vocabulary a scenario has

Everything a scenario can do, in full. The type is
[`Modules`](../crates/brain/src/runtime.rs); a scenario is an `async fn` that
receives one and never mentions Bluetooth.

| Call | What it is for |
| ---- | -------------- |
| `ids()` | Every module, in the order the bench listed them. |
| `descriptor(id)` | What that module said it was. How `arcade` tells the coin slot from the buttons. |
| `set_output(id, channel, on)` | Drive one output. |
| `reset_all()` | Every output off, everywhere. Call it before a round. |
| `next_event(timeout)` | The next event from **any** module, whatever kind. The primitive. |
| `next_press(timeout)` | The next press from any module. Returns *which* module, so a wrong answer is detectable. |
| `next_coin(timeout)` | The next coin from any module, and its pulse count. |
| `wait_for_press(id, timeout)` | A press on **one** module, ignoring presses elsewhere. |
| `drop_pending_presses()` | Discard presses that arrived before now, and say how many. |

Four things about it that change how a game is designed:

**Pick the narrowest wait.** The three filters are all built over
`next_event`, and the choice encodes a rule of your game:
`wait_for_press(target)` *ignores* a press on the wrong module, while
`next_press()` tells you about it so you can call it a miss or end the round.
That difference is the only thing separating whack-a-mole from Simon Says.

**Every wait has a deadline and three ways to fail.** `Timeout` (nobody played),
`ModuleLost` (a module went away), `Closed` (the brain is shutting down). The
runtime reports; the scenario decides. Whack-a-mole carries on without a module
it was not using; Simon Says cannot, because its sequence names specific boxes.
Deciding this per scenario is why the runtime does not decide it for you.

**Some events are levels, some are increments.** A dropped `Measurement` is
corrected by the next one; a dropped `Coin` is money nobody hears about. That is
why they are separate variants -- the reasoning is on
[`Event`](../crates/shared/src/proto.rs) and it is worth reading before designing
anything that counts.

**A press arriving before a round starts is not an answer to it.** Someone
leaning on a button between rounds has a press queued, and it is spent the
instant the next target lights: the box lights and goes dark with nobody
touching it. `drop_pending_presses()` immediately after lighting a target is the
fix; `whack_a_mole` calls it for exactly this reason, and any scenario where a
pause separates one target from the next wants it too.

## Saying what your game needs

A scenario declares its requirements at the top, before the game starts:

```rust
require_role(&modules, &slot, Some(Role::Coin), 1, 1)?;   // a coin slot
require(&modules, id, 1, 1)?;                             // any module, 1 in 1 out
```

This exists because of one specific failure. Waiting for coins from a button
board does not produce an error -- it **hangs**, for the whole timeout, and then
reports that nobody played. Same for an output channel that a module does not
have: the command goes out and nothing happens. `require_role` turns both into a
line at startup naming what it found and what the scenario wanted.

What it checks, in order: the module speaks this protocol version, it is the role
you asked for, and it has *at least* the channels you named. Counts are minimums
-- a scenario needing one button runs happily on a three-button module, using
channel 0.

The one hole: a module that never sent a descriptor is skipped rather than
refused, because an older transport may not carry one. `arcade` therefore refuses
by hand rather than guessing which box is the coin slot.

## Four combinations that already exist

The shipped scenarios are deliberately different *shapes*, not four variations
on one. Find the row that matches the game you have in mind and start from that
one -- they live in
[`crates/brain/src/scenarios/`](../crates/brain/src/scenarios/), one folder each.

| Scenario | Modules it needs | The shape it is an example of |
| -------- | ---------------- | ----------------------------- |
| `whack` | 1+ buttons, interchangeable | **One at a time, at random.** Modules are a pool; nothing distinguishes them; a wrong press costs nothing. |
| `simon` | 1+ buttons, identity matters | **A remembered sequence.** The game refers to specific boxes, so losing one ends the round. |
| `speedrun` | 2+ buttons, order fixed | **A circuit against the clock.** Every module is visited in turn and every split is reported. |
| `arcade` | 1 coin slot **+** 1+ buttons | **Two roles with different jobs:** one module gates the game, the others are the game. Sorted by role, not by id. |

The interesting one is `arcade`, because it is the only one where the modules are
not interchangeable. It is also the pattern most real installations want: a thing
that starts the game (a coin, a big green button, a keypad) and things that
*are* the game.

## Designing one on paper

1. **Write the beats.** "Light comes on. Player hits it. Time is recorded." Each
   beat is an output you drive or an event you wait for -- if a beat is neither,
   no module can produce it.
2. **Assign each beat to a role.** A person pressing → `Button`. A payment or a
   token → `Coin`. Those two are the whole list, and a beat that needs neither
   needs a new module type before it needs a scenario.
3. **Count channels per box, not per game.** Four lights on one enclosure is
   `outputs: 4` on one module; four boxes is four modules. The second costs four
   BLE connections and four power supplies, and the first cannot be spread across
   a pitch.
4. **Decide what a death means.** For every module, answer: if this one drops
   out mid-round, does the round continue, restart, or stop? That answer is a
   `match` arm on `WaitError::ModuleLost`, and writing the game without it is how
   you end up scoring rounds against a box that is no longer there.
5. **Decide what a timeout means.** A round that nobody answers has to end
   somehow. `PATIENCE` in the shipped scenarios is 30 s.
6. **Play it in simulation.** `just simulate --scenario <yours>` before wiring
   anything. Typing `-a` makes a module drop out, which is how step 4 gets
   tested.

Then copy the closest scenario: [Getting started](GETTING-STARTED.md) has the
shortest possible one, and the real ones are next to it in `scenarios/`.

## What the brain does not do

Four things a design must not assume, because they decide what a game can
promise:

- **A lost module does not come back mid-scenario.** `ModuleLost` is reported and
  the round can react to it, but that board is only re-acquired when the scenario
  ends and acquisition runs again. A game that has to survive a box dying should
  end its round and start another.
- **Timing is not certified.** Splits are measured on the laptop, and BLE adds
  delay between the hit and the brain hearing about it -- worse, *variable* delay.
  Reporting a reaction time to the millisecond would be a lie until that jitter
  is measured; see [why modit exists](WHY.md).
- **Coins do not work over the radio.** `arcade` is playable under `--simulate`
  only, for the reason given under the table above.
- **Roles are a closed set.** A new module type means adding a variant to
  [`shared::proto`](../crates/shared/src/proto.rs) and writing firmware. There is
  no configuration file that invents one, deliberately: a scenario is Rust, so the
  thing it addresses is a type.
