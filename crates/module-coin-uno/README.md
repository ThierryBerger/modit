# module-coin-uno

Arduino Uno firmware for a coin acceptor, standalone: it counts each coin's
pulses and blinks the on-board `L` LED once per pulse. **Works on real
hardware:** each coin blinks exactly its taught pulse count, repeatably, and the
solenoid firing does not reset the board. It has no radio and **does not talk to
the brain**; that is work in progress.

- **Wiring, power and the checks to run first:**
  [`docs/hardware/module-coin.md`](../../docs/hardware/module-coin.md), Arduino
  Uno tabs. **Read it before connecting anything** -- the acceptor runs on 12 V,
  and USB is never plugged in while that supply is.
- **Toolchain:** `just setup-avr` (Debian/Ubuntu). The pinned nightly installs
  itself from `rust-toolchain.toml`.
- **Flash it:** unplug the 12 V supply, plug in USB, `just flash-coin-uno` from
  the repository root. It opens the serial console afterwards; Ctrl+C closes it.

The pulse-counting rules are [`shared::input`](../shared/src/input.rs) and
[`shared::coin`](../shared/src/coin.rs), the same ones the ESP32 firmware uses.
