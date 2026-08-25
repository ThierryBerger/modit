# 06 — Make the firmware fail loudly instead of silently

**Audit:** 3, 4, 10, 24 · **Goal:** fail-free run + good error messages · **Size:** small

## Why

The board currently has the worst possible failure mode: `#[panic_handler]` is
`loop {}` (`buttons/src/main.rs:38-41`), so a panic stops the module dead with no
output, no reset, and no indication anything happened. From across a room it is
indistinguishable from a flat battery or a BLE dropout — and there are two
reachable panics sitting right next to it.

## Current state

- `buttons/src/main.rs:38-41` — `fn panic(_info: &PanicInfo) -> ! { loop {} }`.
  `esp-backtrace` is a dependency with the `println` feature, but the local handler
  wins.
- `buttons/src/main.rs:96` — `if data[0] == 0` in the LED write callback. A
  zero-length write panics. Any BLE client can send one.
- `buttons/src/main.rs:103-109` — `rf3` does `data[..len].copy_from_slice(bytes)`
  with `len = "COIN:20".len()`. Panics if the read buffer is shorter. The whole
  callback is dead placeholder content.
- `buttons/src/main.rs:140` — debounce counts loop iterations
  (`tick.saturating_sub(button_last_pushed_tick) > 5`), so the window varies with
  BLE load rather than being a fixed duration.
- `buttons/src/main.rs:84` — `Uuid` is used but never imported; it resolves only
  because `gatt!` at line 110 expands to `use bleps::att::Uuid;` in the same block.

## Target state

A panic prints a backtrace over serial and resets the chip. Malformed input from
the radio is ignored with a log line, never fatal. Debounce is expressed in
milliseconds.

## Steps

- [x] Delete the local `#[panic_handler]` and let `esp-backtrace` provide it.
      It is already a dependency with the `println` feature. Add the
      `panic-handler` and `exception-handler` features if they are not default,
      and consider `esp-backtrace`'s reset-on-panic behaviour so a module in the
      field recovers by rebooting rather than hanging.
- [~] Verify by inducing a deliberate panic and watching `espflash --monitor`.
      **Do this before the bounds fixes** — otherwise you cannot tell whether the
      fixes worked.
      **NOT DONE — needs a board on the desk. See Notes.**
- [x] `wf2`: handle `data` defensively —
      `match data.first() { Some(0) => led.set_low(), Some(_) => led.set_high(),
      None => println!("empty write to led_charac, ignoring") }`.
- [x] `rf3`: either delete it (the read is unused by `brain`) or make it the
      identity/config endpoint from plan 05. Either way, bound the copy:
      `let n = bytes.len().min(data.len());`.
- [x] Replace the tick-based debounce with `time::Instant` — the `now` closure at
      `buttons/src/main.rs:71` already gives milliseconds since epoch. A fixed
      ~50 ms window is the conventional starting point.
- [x] Add the explicit `use bleps::att::Uuid;` import (also listed in plan 02).
- [~] Consider a heartbeat: a slow LED blink, or a periodic `println!` with uptime,
      so "the board is alive but not connected" is visually distinguishable from
      "the board is dead". This is the cheapest possible field-debugging tool.
      **Not done — considered and deferred, see Notes.**

## Done when

- [~] A deliberate `panic!()` in the firmware prints a backtrace over serial and the
      chip resets. **Verified at link level only — needs hardware.**
- [x] A zero-length write to the LED characteristic is logged and ignored.
- [x] A read of the button characteristic with a 4-byte buffer does not panic.
- [~] Holding the button produces exactly one notification regardless of BLE load.
      **Logic is time-based now; needs hardware to confirm the 50 ms window.**
- [x] Every name used in `main.rs` has a visible `use` for it.

## Notes

Code done 2026-08-25. **Hardware verification outstanding — see below.**

### The panic handler

The audit was right about the cause: `panic-handler` and `exception-handler` are
**not** default features of `esp-backtrace` (only `colors` is). That is why the
local `loop {}` handler compiled without a duplicate-lang-item error, and why it
was the one that ran.

Both features are now enabled. But esp-backtrace's own handler prints the
backtrace and then calls `halt()`, which stalls the cores and spins — visible over
serial, but the board still never comes back. Since a module in an escape game has
nobody to power-cycle it, the `custom-halt` feature is enabled too and
`custom_halt()` in `main.rs` calls `esp_hal::system::software_reset()`.

Net panic behaviour: **print the panic message → print the backtrace → print
"rebooting after panic" → reset.** Loud *and* self-recovering.

### The read callback

`rf3` was deleted rather than bounded. It served a hardcoded `"COIN:20"` that
nothing reads — `brain` only ever subscribes for notifications. It is now
`|_offset, _data| 0` (zero bytes read), which cannot panic regardless of the
client's buffer size, with a `TODO(plan-05)` noting this is where module identity
belongs. Both callbacks were renamed `wf2`/`rf3` → `write_led`/`read_button`.

### Debounce

Now `now().saturating_sub(button_last_pressed_ms) > DEBOUNCE_MS` with
`DEBOUNCE_MS = 50`, using the `now` closure that already existed for the HCI
connector. The rising-edge detection was also pulled out into a named
`rising_edge` binding — the old version called `button.is_high()` twice per
iteration and updated `button_last_pushed_tick` on *every* high sample, not just
on the edge, which quietly meant a held button kept pushing the deadline out.

The `tick` counter is gone entirely.

### Heartbeat: considered, not done

Deferred deliberately. It overlaps with plan 03's logging work and with plan 05's
identity work (a heartbeat is much more useful once it can say *which* module it
is). Revisit when doing 05.

### Verification status — read this before flashing

Verified on this machine:

- `cargo build --release` links, and `nm` shows `custom_halt` as a defined symbol
  (`T`) — so the `custom-halt` hook resolves rather than silently not being called.
- esp-backtrace's `PANIC` banner and `Backtrace:` strings are present in the
  linked binary, confirming its handler is the one included.
- `cargo clippy -- -D warnings` is clean.

**Not verified — needs a board:**

1. Induce a deliberate `panic!()`, confirm the backtrace prints and the chip
   actually resets rather than hanging.
2. Confirm a zero-length write to `led_charac` logs and is ignored.
3. Confirm one press produces exactly one notification, and that 50 ms is the
   right window for the physical button in use.

Item 1 is the important one: the whole plan rests on it.
