# Modit

**Documentation: <https://vrixyz.github.io/modit/>** — start there if you are
deciding whether modit suits you. It publishes the pages in [`docs/`](docs/) and
[`plans/`](plans/) with nothing added. *Live from the first push to `main` after
Pages is set to "GitHub Actions" in the repository settings; until then, read the
markdown here.*

Modit helps with creating custom logic for multiple embedded devices (modules) communicating together,
through a "central" bluetooth client (brain).

This uses bluetooth (LE) as a communication protocol, for the following advantages:

- **No network infrastructure.** No venue wifi, no router, no internet. Power a
  module and it is reachable. This is the one that matters: it turns setup from an
  installation into unpacking a bag.
- **No wiring between modules.** They only need power.
- **Low energy per message, and low idle draw**, which is what makes
  battery-powered modules plausible.
- **A single "brain" as a client**, running higher level code a user tweaks to
  their needs.

### Honest limits

BLE is not free of trade-offs, and it is better to know them up front than to
discover them mid-build:

- **Range** is realistically around 10 m indoors, less through walls. Modules
  spread across several rooms may not all reach one brain.
- **Simultaneous connections** are capped by whatever BLE stack the brain runs on
  — typically single digits to low double digits, with throughput degrading before
  the cap. Fine for a handful of modules; a 15-prop room needs thinking about.
- **Battery life depends far more on the firmware than on the radio.** This
  firmware never sleeps, so it does not collect most of BLE's power advantage.
- **A module that can be commanded cannot sleep deeply**, because it has to stay
  reachable. That is a consequence of being bidirectional, not of BLE — a
  notify-only sensor can be dramatically lower-power than one with an actuator.

## Target audience

Games that have to be **set up and taken down**, not installed.

A fixed escape room can afford wires -- it is built once and lives in a building.
Modit targets the case where there is no building, no time, or no permission to
touch it: sport training on a pitch, outdoor and nomadic games, a conference
stand, a museum room you are not allowed to drill, a festival.

A module needs power and nothing else, so setup is unpacking a bag rather than an
installation. The permanent install still works -- it is the easier case, and
designing for the harder one covers it.

The reasoning, and what follows from it, is in [docs/WHY.md](docs/WHY.md).

## Naming

Modit (mod it), is a reference to its modularity, containing:

1. mods: different low-level modules, which should be once developped and then deployed and widely useful without changes.
2. a moddable "brain", mod it!

"moddable" here refers to writing custom Rust.

## Status

Early. Two module types exist: a button with an LED (`crates/module-button`),
which works on real hardware, and a coin acceptor, which counts real coins on an
Arduino Uno (`crates/module-coin-uno`, standalone) and has ESP32 firmware
(`crates/module-coin`) written but not yet run. The brain ships four scenarios.
The edges are rough, and [`plans/`](plans/) is where the remaining work and the
reasoning behind it live.

The coin module plays under `--simulate` but **not** over BLE: the BLE link is
still typed to button modules and speaks that firmware's own wire format.

## How to

Today, in practice:

- Try it with no hardware at all: `just simulate`, or
  `just simulate --scenario arcade` to play the coin-operated one (type `$` to
  post a coin).
- Wire an ESP32: button on GPIO33 (pull-down), LED on GPIO26.
- Flash it: `just flash a` -- needs the `esp` toolchain, run `just setup` first.
- Run the brain: `just brain --modules a`. The default bench is two boards
  (`a,b`); `--modules` names the ones you actually built.

The coin acceptor is a separate build (`just flash-coin-uno` for the Uno,
`just flash-coin slot` for the ESP32) and involves 12 V. Read [hardware](docs/HARDWARE.md) before wiring it -- the wrong acceptor wire
on a pin will destroy it.

Run `just` on its own to see every available command.

Start with **[getting started](docs/GETTING-STARTED.md)** to have it running in
a minute, the **[tutorial](docs/TUTORIAL.md)** for the full walkthrough,
[hardware](docs/HARDWARE.md) for wiring, and
**[composing](docs/COMPOSING.md)** to design a game out of modules -- what each
module offers a scenario, and what the brain will refuse.

### Defining modules

Module UUIDs live in `shared::uuids`, which `brain` references directly. The
firmware duplicates them as string literals because the `gatt!` macro needs
literals; a test fails if the two drift apart.

There is deliberately **no config-file workflow** -- a scenario is Rust, and so
are its UUIDs. A module type is a Rust type, not a file the program parses at
startup, so a typo is a compile error rather than a scan that finds nothing.

## Repository layout

| Path | What |
| ---- | ---- |
| `crates/shared` | Module definition types, shared between host and firmware (`no_std`). |
| `crates/brain`  | The host binary: scans, connects, runs the scenario. |
| `crates/module-button`| ESP32 firmware for a button+LED module. Separate workspace -- different toolchain and target. |
| `crates/module-coin` | ESP32 firmware for a coin acceptor. Separate workspace, same reason. Speaks `shared::proto`. |
| `crates/module-coin-uno` | Arduino Uno firmware for the same acceptor, standalone: counts coins, blinks an LED. Separate workspace. |
| `plans/`        | Planned work, and the reasoning that chose it. Start with [`plans/README.md`](plans/README.md). |
| `docs/`         | [Why](docs/WHY.md), [getting started](docs/GETTING-STARTED.md), [composing](docs/COMPOSING.md), [tutorial](docs/TUTORIAL.md), [hardware](docs/HARDWARE.md), [architecture](docs/ARCHITECTURE.md). |
| `docs/site/`    | How those pages become <https://vrixyz.github.io/modit/>. One staging script, no second copy of any page. |
