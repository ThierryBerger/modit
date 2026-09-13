# module-coin

ESP32 firmware for a coin acceptor module: coins in, credits for a scenario to
spend. Written and building, but **it has not met an acceptor yet**, and the
brain's BLE link cannot drive it: that link still speaks `module-button`'s own
wire format. It runs today under `just simulate --scenario arcade`.

- **Wiring, bill of materials and the schematics:**
  [`docs/hardware/module-coin.md`](../../docs/hardware/module-coin.md). **Read it
  before connecting anything** — the acceptor runs on 12 V, and the pulse line
  will destroy a GPIO if your unit pulls it up.
- **What a scenario can do with it:**
  [`docs/COMPOSING.md`](../../docs/COMPOSING.md).
- **Flash it:** `just flash-coin slot` from the repository root (`slot` is the id
  the `arcade` scenario looks for).

Unlike `module-button`, this crate speaks [`shared::proto`](../shared/src/proto.rs):
one command characteristic, one event characteristic, and a `Descriptor` it sends
on connect.
