# Modit

Modit helps with creating custom logic for multiple embedded devices (modules) communicating together,
through a "central" bluetooth client (brain).

This uses bluetooth (LE) as a communication protocol, for the following advantages:

- **No network infrastructure.** No venue wifi, no router, no internet. Power a
  module and it is reachable. Setting this up outdoors is possible.
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
- **Battery life depends far more on the firmware than on the radio.** The
  current firmware never sleeps, so it does not yet collect most of BLE's power
  advantage. See [plan 10](plans/todo/10-power-and-battery.md), which starts by
  measuring rather than assuming.
- **A module that can be commanded cannot sleep deeply**, because it has to stay
  reachable. That is a consequence of being bidirectional, not of BLE — a
  notify-only sensor can be dramatically lower-power than one with an actuator.

## Target audience

Made for escape games, this enables automating complex scenarios without the need of manual interaction.

## Naming

Modit (mod it), is a reference to its modularity, containing:

1. mods: different low-level modules, which should be once developped and then deployed and widely useful without changes.
2. a moddable "brain", mod it!

"moddable" here refers to writing custom Rust.

## Status

Early. One module type exists (a button with an LED, `crates/module-button`) and the
brain runs a hardcoded whack-a-mole scenario. It works on real hardware, but the
edges are rough -- see [`plans/`](plans/) for the audit and the ordered work.

## How to

Today, in practice:

- Wire an ESP32: button on GPIO33 (pull-down), LED on GPIO26.
- Flash it: `just flash` -- needs the `esp` toolchain, run `just setup` first.
- Run the brain: `just brain`.

Run `just` on its own to see every available command.

Start with the **[tutorial](docs/TUTORIAL.md)** for the full walkthrough, and
[hardware](docs/HARDWARE.md) for wiring.

### Defining modules

Module UUIDs live in `shared::uuids`, which `brain` references directly. The
firmware duplicates them as string literals because the `gatt!` macro needs
literals; a test fails if the two drift apart.

There is deliberately **no config-file workflow** -- a scenario is Rust, and so
are its UUIDs. The reasoning is recorded in
[plan 07](plans/done/07-typed-module-definitions.md).

## Repository layout

| Path | What |
| ---- | ---- |
| `crates/shared` | Module definition types, shared between host and firmware (`no_std`). |
| `crates/brain`  | The host binary: scans, connects, runs the scenario. |
| `crates/module-button`| ESP32 firmware for a button+LED module. Separate workspace -- different toolchain and target. |
| `plans/`        | Audit and planned work. Start with [`plans/README.md`](plans/README.md). |
| `docs/`         | [Tutorial](docs/TUTORIAL.md), [hardware](docs/HARDWARE.md), [architecture](docs/ARCHITECTURE.md). |
