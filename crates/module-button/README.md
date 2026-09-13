# module-button

ESP32 firmware for a modit button module: one button, one LED. The module every
other part of this project was built against, and the one that works on real
hardware.

- **Wiring, bill of materials and the schematic:**
  [`docs/hardware/module-button.md`](../../docs/hardware/module-button.md) — the
  net table there is the source of truth, and a test in `brain` keeps it in step
  with the pins this crate asks for.
- **What a scenario can do with it:**
  [`docs/COMPOSING.md`](../../docs/COMPOSING.md).
- **Flash it:** `just flash a` from the repository root, where `a` is the id this
  board answers to. Needs the `esp` toolchain — `just setup` once, then
  `source ~/export-esp.sh` per shell.

Its own workspace, because the firmware needs a different toolchain and target
than the host crates; the root `Cargo.toml` explains why that cannot be merged.
