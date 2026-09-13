# module-coin

ESP32 firmware for a coin acceptor module: coins in, credits for a scenario to
spend. **Work in progress:** written and building, but **it has not met an
acceptor yet**, and the brain's BLE link cannot drive it: that link still speaks
`module-button`'s own wire format. It runs today under
`just simulate --scenario arcade`.

The acceptor side is not in doubt: [`module-coin-uno`](../module-coin-uno/) counts
real coins with the same wire, switch setting and pulse-timing rules this crate
uses.

- **Wiring, bill of materials and the schematics:**
  [`docs/hardware/module-coin.md`](../../docs/hardware/module-coin.md). **Read it
  before connecting anything** — the acceptor runs on 12 V, and the wrong wire
  on a GPIO will destroy it.
- **What a scenario can do with it:**
  [`docs/COMPOSING.md`](../../docs/COMPOSING.md).
- **Flash it:** `just flash-coin slot` from the repository root (`slot` is the id
  the `arcade` scenario looks for).

Unlike `module-button`, this crate speaks [`shared::proto`](../shared/src/proto.rs):
one command characteristic, one event characteristic, and a `Descriptor` it sends
on connect.
