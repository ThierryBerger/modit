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

## How to

- Create a `.ron` file for each instance of modules needed.
- Flash your modules with their relevant code, loading their respective asset at compile time.
- Develop your custom brain, using the same assets.
- Run your custom brain.
- Enjoy!
