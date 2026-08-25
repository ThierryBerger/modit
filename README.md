# Modit

Modit helps with creating custom logic for multiple embedded devices (modules) communicating together,
through a "central" bluetooth client (brain).

This uses bluetooth (LE) as a communication protocol, for the following advantages:

- Doesn't need wifi available.
  - Simpler to setup
  - Setting this up outside is possible
- Doesn't need wiring together, modules can be fairly far away from each other.
- doesn't draw a lot of power, so battery-friendly, for portable setups.
- A single "brain" as a client, higher level code a user tweaks to its needs, communicating with modules.

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
