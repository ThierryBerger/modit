# Getting started with modit

Modit runs games across several small wireless boxes. Each box is an ESP32 with
a button and a light. A laptop runs the game logic and talks to them over
Bluetooth LE.

A module needs power and nothing else. No wiring between boxes, no venue wifi, no
router. That makes it suited to games you set up and take down rather than
install: sport training, outdoor games, a conference stand, a museum, a festival.
A permanent escape room works too, and is the easier case.

You do not need any hardware to try it.

## Run it with no hardware

You need Rust and [`just`](https://github.com/casey/just).

```
git clone <this repo>
cd modit
just simulate
```

Two simulated modules appear, named `a` and `b`. Type a module's name and press
enter to press its button.

```
 INFO  brain > running in simulation -- no radio, no boards
 INFO  brain > simulator ready:
 INFO  brain >   a       press module a's button
 INFO  brain >   $       post a 1-pulse coin into the slot
 INFO  brain >   -a      make module a drop out, as if it lost power
 INFO  brain >   q       quit
 INFO  brain::runtime > all 2 module(s) bound, starting the scenario
 INFO  brain::link::sim > [sim] b output 0 -> ON
 INFO  brain::scenarios > module b is lit
```

Module `b` is lit. Type `b` and enter:

```
 INFO  brain::scenarios > module b hit
 INFO  brain::link::sim > [sim] b output 0 -> off
 INFO  brain::link::sim > [sim] a output 0 -> ON
 INFO  brain::scenarios > module a is lit
```

That is whack-a-mole. One module lights at random, you press it, the next one
lights.

Type `-a` to make a module lose power mid-game, and watch the scenario carry on
without it. Type `q` to quit.

## The other games

```
just simulate --scenario simon
just simulate --scenario speedrun
just simulate --scenario arcade
```

| Scenario | What it does |
| -------- | ------------ |
| `whack` | One module lights at random. Press it. Repeats forever. This is the default. |
| `simon` | A growing sequence plays back. Repeat it from memory. A wrong press ends the round. |
| `speedrun` | Ten timed laps between modules. Reports every split and counts misses. |
| `arcade` | Pay a coin, play a round. Type `$` to post a coin, or `$3` for a 3-pulse one. |

`arcade` uses a coin acceptor module. That module runs in simulation but has not
met real hardware yet.

## Add real boards

You need an ESP32 board with a push button and an LED. Wiring is in
[docs/hardware/module-button.md](hardware/module-button.md): the button goes to
3V3 on GPIO33, the LED through a resistor on GPIO26.

Two boards make whack-a-mole a hunt rather than a reaction test, so the rest of
this page uses two. **One is enough to play** -- see the note at the end.

Install the ESP toolchain once:

```
just setup
source ~/export-esp.sh
```

Flash each board with its own id. The id is baked into the binary, so every
board gets its own flash:

```
just flash a
just flash b
```

Each board runs a self-test at boot. It blinks the LED three times and reports
whether the button reads low at rest:

```
self-test: blinking the LED three times (GPIO26)
self-test: button (GPIO33) reads low at rest, as expected
```

If it says the button reads HIGH at rest, the button is wired to GND instead of
3V3. That is the most common mistake, and the self-test is there to catch it
before Bluetooth is involved.

Then run the game:

```
just brain
```

The brain scans, finds `modit-button-a` and `modit-button-b`, and starts the
same scenario you already played in simulation.

### With only one board

The default bench is `a,b`, and the brain will not start until every board on it
is bound -- so with one board it rescans forever, naming the one it cannot find.
Tell it what you actually built:

```
just brain --modules a
```

Whack-a-mole then lights the same module every round, which makes it a reaction
timer rather than a hunt. `simon` works the same way. `speedrun` refuses, and
says why: there is nothing to move between.

## Write your own game

A scenario is an async function. It never mentions Bluetooth, a characteristic,
or a task. Abridged from the real `whack_a_mole`, which also handles timeouts and
modules dropping out:

```rust
pub async fn whack_a_mole(modules: Arc<Modules>) -> anyhow::Result<()> {
    let ids = modules.ids().to_vec();
    modules.reset_all().await?;

    loop {
        let target = ids[random_index(&ids)].clone();
        modules.set_output(&target, 0, true).await?;

        modules.wait_for_press(&target, PATIENCE).await?;

        modules.set_output(&target, 0, false).await?;
    }
}
```

The same code runs against real boards and against `--simulate`. That is what
makes the simulator useful as a test harness: the scenario cannot tell the
difference.

Scenarios live in [`crates/brain/src/scenarios.rs`](../crates/brain/src/scenarios.rs).
Copy whichever one waits the way yours needs to:

- `whack_a_mole` waits on one specific module and ignores wrong presses.
- `simon_says` waits on any module, and a wrong press ends the round.
- `speedrun` waits on any module and counts wrong presses as misses.

Before writing one, read [composing a game](COMPOSING.md): what each module type
offers, the whole vocabulary a scenario has, and how a scenario says what it needs
so a mismatch is an error at startup instead of a game that hangs.

## How it fits together

```
scenario          your game logic, plain async Rust
   |
Modules           set_output, next_press, wait_for_press
   |
Link              BLE, or an in-process channel for --simulate
   |
module firmware   ESP32, one button, one LED
```

Everything above `Link` deals in messages, not radios. Swapping the transport is
a trait implementation, not a rewrite.

| Crate | What |
| ----- | ---- |
| `crates/shared` | Message types shared by laptop and firmware. `no_std`. |
| `crates/brain` | The laptop binary: scans, connects, runs the scenario. |
| `crates/module-button` | ESP32 firmware for a button and an LED. |
| `crates/module-coin` | ESP32 firmware for a coin acceptor. |

## Commands

```
just                 list every recipe
just simulate        run a scenario with no hardware
just brain           run against real boards
just flash a         build, flash and monitor one board
just monitor         watch an already-flashed board
just test            run the host tests
just ci              everything CI runs
```

## Limits

- Range is about 10 m indoors, less through walls.
- Bluetooth LE caps simultaneous connections in the single digits to low double
  digits, and throughput degrades before the cap. A handful of modules is fine.
  Fifteen needs thought.
- The firmware never sleeps, so battery life is currently worse than the radio
  allows.

## Where to go next

- [Why](WHY.md) — what this is for, and what follows from it.
- [Composing a game](COMPOSING.md) — designing one out of modules, on paper.
- [Full tutorial](TUTORIAL.md) — the long walkthrough.
- [Hardware](HARDWARE.md) — wiring for every module.
- [Architecture](ARCHITECTURE.md) — how the layers fit.
- [Plans](../plans/README.md) — what is built, what is next, and why.
