# Audit — 2026-08-25

> **Update 2026-08-25:** plans 01, 02, 03 and 06 have been applied. Findings
> **3, 4, 8, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 23, 24, 25, 27** are
> resolved; **7** and **26** are partially resolved.
> Resolved items are marked ~~struck through~~ below and kept for the record.
> Everything unmarked is still true of the code today.

Full read-through of every source file. Both crates were verified to compile
(`brain` on stable/host, `buttons` on the `esp` toolchain for `xtensa-esp32-none-elf`).

## What the project actually is

```
shared/   no_std types: Notifier { service, charac_notify_id }
          and Writable { service, charac_write_id }. Just UUID string pairs.
             |
   +---------+---------+
   |                   |
buttons/            brain/
ESP32 firmware      host binary (tokio + btleplug)
no_std, bleps       scans, connects, plays a whack-a-mole game
```

- **`buttons`** advertises as `modit-esp32-2`, exposes one GATT service
  (`937312e0-…cf30`) with a write characteristic driving an LED on **GPIO26** and a
  notify characteristic fired by a button on **GPIO33** (pull-down).
- **`brain`** scans for peripherals whose local name starts with `modit`, connects,
  discovers services, and matches each `Notifier`/`Writable` definition against the
  discovered characteristics. It then runs a whack-a-mole loop: light one LED,
  wait for that button, light another.

The three-crate split exists because `buttons` needs a different toolchain
(`channel = "esp"`) and target (`xtensa-esp32-none-elf`) than `brain`.

## Findings

Severity: **[H]** breaks or will break in the field · **[M]** costs real time ·
**[L]** papercut.

### Correctness

1. **[H] `is_connected()` never detects a clean disconnect.**
   [`brain/src/main.rs:40-52`](../crates/brain/src/main.rs#L40-L52) — the
   `tokio::select!` discards the result of `peripheral.is_connected()` with `_`, so
   a peripheral that resolves `Ok(false)` takes the `true` branch. Disconnection is
   only ever noticed via the 1-second timeout. The self-healing reconnect logic is
   therefore mostly dead.

2. **[H] Notifications are dropped by design.**
   [`ble/read.rs:10`](../crates/brain/src/ble/read.rs#L10) opens a **new**
   notification stream on every poll, waits 200 ms, and drops it. Anything arriving
   in the gap between polls, or buffered in the dropped stream, is lost. A BLE
   notification stream is an event source; it is being polled like a register.

3. ~~**[H] A panic on the ESP32 hangs the board silently.**~~ — **fixed (plan 06).**
   [`buttons/src/main.rs:38-41`](../crates/buttons/src/main.rs#L38-L41) defines
   `fn panic(_) -> ! { loop {} }`, which wins over `esp-backtrace`'s handler. No
   message, no backtrace, no reboot — the board just stops. This is the worst
   possible failure mode for a device with no debugger attached, and it is reachable
   from two unchecked indexes below.

4. ~~**[H] Two unchecked indexes in the firmware.**~~ — **fixed (plan 06).**
   `wf2` reads `data[0]` ([`buttons/src/main.rs:96`](../crates/buttons/src/main.rs#L96))
   — a zero-length write panics. `rf3` does
   `data[..len].copy_from_slice(bytes)` ([`:107`](../crates/buttons/src/main.rs#L107))
   — panics if the read buffer is shorter than `"COIN:20"`. Combined with finding 3,
   a malformed packet from any BLE client bricks the module until power-cycled.

5. **[M] Modules have no stable identity.**
   Every board advertises the same name and the same service UUID. `brain` builds its
   module list by discovery order ([`ble/mod.rs:116-127`](../crates/brain/src/ble/mod.rs#L116-L127)),
   so "button 0" is a different physical box on every run. For an escape game where a
   scenario says "the red button", this is a blocker.

6. **[M] The RNG is seeded with a constant.**
   `SmallRng::seed_from_u64(42)` ([`brain/src/main.rs:88`](../crates/brain/src/main.rs#L88))
   — every run plays the identical sequence.

7. **[M] The re-arm task holds the RNG lock across a sleep.** *(fixed in plan 03 as a drive-by: the delay and target index are taken while holding the lock, which is then dropped before the sleep. The wider concurrency rework is still plan 04.)*
   [`brain/src/main.rs:135-148`](../crates/brain/src/main.rs#L135-L148) —
   `rng.lock().await` is acquired, then the task sleeps 500–1000 ms while holding it.
   Any other task touching the RNG stalls for the full duration.

8. ~~**[M] `join!(t)` in a loop is a sequential await.**~~ — **fixed (plan 03):** `join_all`, and `JoinError` is logged with the module index.
   [`brain/src/main.rs:157-159`](../crates/brain/src/main.rs#L157-L159) —
   `futures::join!` with one argument is just `t.await`, and `let _ =` swallows the
   `JoinError`. A task that panicked is indistinguishable from one that finished.

9. **[L] Division by zero if the module list is empty.**
   `rng.next_u64() % buttons.len() as u64` ([`brain/src/main.rs:91`](../crates/brain/src/main.rs#L91)).
   Unreachable today, but only because `modules` is a hardcoded 2-element `vec!`.

10. ~~**[L] Tick-based debounce.**~~ — **fixed (plan 06):** now a 50 ms window.
    [`buttons/src/main.rs:140`](../crates/buttons/src/main.rs#L140) counts iterations
    of the BLE work loop, so the debounce window varies with radio load rather than
    being a fixed duration.

### Failure modes and error messages

11. ~~**[H] `unwrap()` on every BLE operation.**~~ — **fixed (plan 03).** The game-loop writes now log and restart the round; making them per-module retries is plan 04. `brain/src/main.rs` lines
    69, 70, 73, 84, 95, 129, 146; `ble/mod.rs` lines 34, 49, 97, 132. Any transient
    radio hiccup — a device that walks out of range between the scan and the reset
    write — aborts the whole process with a backtrace instead of a retry.

12. ~~**[H] `Uuid::parse_str(…).unwrap()` runs per characteristic, per peripheral.**~~ — **fixed (plan 03):** `validate_modules` runs before any radio work, and parsing is now once per peripheral.
    [`ble/mod.rs:34`](../crates/brain/src/ble/mod.rs#L34) and
    [`:49`](../crates/brain/src/ble/mod.rs#L49). A single typo in a module definition
    panics deep inside a scan loop, with a message that names neither the module nor
    the offending string. UUIDs should be parsed once, up front.

13. ~~**[H] Error messages describe actions the code does not take.**~~ — **fixed (plan 03).**
    `"No Bluetooth adapters found"` ([`ble/mod.rs:68`](../crates/brain/src/ble/mod.rs#L68))
    is printed and then execution continues into a loop over zero adapters, returning
    an all-`None` result. `"BLE peripheral devices were not found, sorry. Exiting..."`
    ([`:82`](../crates/brain/src/ble/mod.rs#L82)) does not exit. Neither says what to
    check or what to do.

14. ~~**[M] The retry loop is silent about why it is retrying.**~~ — **fixed (plan 03):** it names the unbound modules.
    [`brain/src/main.rs:75-78`](../crates/brain/src/main.rs#L75-L78) — if any module
    is missing it `continue`s without naming which one, so the console shows a 3-second
    scan cycle repeating forever with no diagnosis.

15. ~~**[M] `pretty_env_logger` is initialised and then never used.**~~ — **fixed (plan 03).**
    [`brain/src/main.rs:56`](../crates/brain/src/main.rs#L56) — every message in the
    codebase is a raw `println!`/`eprintln!`. `RUST_LOG` has no effect, and the
    per-characteristic `"Checking notifier characteristic …"` chatter cannot be turned
    off.

16. ~~**[L] `dbg!(adapter_list)` left in.**~~ — **fixed (plan 03).** Note the same trap recurred via `Adapter`'s derived `Debug`; both now use `adapter_info()`.
    [`ble/mod.rs:73`](../crates/brain/src/ble/mod.rs#L73).

### Structure and onboarding

17. ~~**[H] No cargo workspace.**~~ — **fixed (plan 01).** Three crates, three `Cargo.lock`s, three identical
    `.gitignore`s, three `target/` dirs. `cargo build` at the repo root does nothing.
    There is no single command that checks the project. (`buttons` genuinely cannot
    share a workspace with `brain` — different toolchain and target — but `brain` +
    `shared` can, and the split needs to be documented rather than accidental.)

18. ~~**[H] `assets/` is dead, and wrong.**~~ — **fixed (plan 02):** deleted. Nothing in any crate reads it — no
    `include_str!`, no `ron::`. Worse, the contents contradict the code:
    - `simple_button.ron` declares `937312e0-…cf38` / `987312e0-…cf38`; the firmware
      and `brain` actually use the `…cf30` family.
    - `simple_writable.ron` is typed `Buttons(service:, charac_notify_id:)` but
      `Writable` has fields `service` and `charac_write_id` — it would not deserialize.
    - `simple_notifier.txt` / `simple_writable.txt` are bare UUIDs with no consumer.

19. ~~**[H] The README documents a feature that does not exist.**~~ — **fixed (plan 02).** The "How to" section
    describes creating a `.ron` per module and loading it at compile time. The real
    entry point hardcodes the UUIDs inline at
    [`brain/src/main.rs:57-66`](../crates/brain/src/main.rs#L57-L66). This is the
    single most expensive trap for a returning reader.

20. **[H] The README has no build, flash, or run instructions.** No `espup`, no
    `espflash`, no toolchain note, no wiring, no `RUST_LOG`. The hardware pinout
    (GPIO33 button, GPIO26 LED) exists only at
    [`buttons/src/main.rs:62-69`](../crates/buttons/src/main.rs#L62-L69).

21. **[M] `&'static str` in `shared` blocks the promised RON workflow.**
    [`shared/src/lib.rs:10,17,25,29`](../crates/shared/src/lib.rs#L10) — a
    runtime-deserialized RON cannot produce `&'static str`. The `// TODO: use proper
    type` comments mark exactly this. The types also model a UUID as a string, so
    invalid UUIDs are representable.

22. **[M] There is no seam for the "moddable brain" the README promises.** The game
    logic is interleaved with BLE plumbing inside `main()`
    ([`brain/src/main.rs:54-160`](../crates/brain/src/main.rs#L54-L160)). Writing a new
    scenario means editing `main`, not implementing something.

23. ~~**[M] Unused dependencies.**~~ — **fixed (plan 01).** `ron` in `brain/Cargo.toml`, `dotenvy_macro` in
    `buttons/Cargo.toml`. The latter, plus a gitignored `.env` and an empty `[env]`
    block in `buttons/.cargo/config.toml`, implies a configuration mechanism that was
    started and abandoned. Nothing documents what variable was expected.

24. ~~**[M] Invisible dependency in the firmware.**~~ — **fixed (plan 02):** the call site now uses the fully qualified `bleps::att::Uuid`. A plain `use` could not work: `gatt!` expands to the same import in the same block and shadows it. `Uuid` at
    [`buttons/src/main.rs:84`](../crates/buttons/src/main.rs#L84) is never imported —
    it only resolves because the `gatt!` macro at line 110 expands to
    `use bleps::att::Uuid;` in the same block. Moving or removing the `gatt!` block
    breaks line 84 with an error that points nowhere near the cause.

25. ~~**[L] Stale example header.**~~ — **fixed (plan 02).** [`buttons/src/main.rs:1-8`](../crates/buttons/src/main.rs#L1-L8)
    is the unmodified esp-hal example banner: it advertises "three characteristics"
    (there are two) and carries `//% FEATURES:` / `//% CHIPS:` build-matrix markers
    that mean nothing here.

26. **[L] The hardcoded `-2` in the advertised name.** *(partially addressed — now carries a `TODO(plan-05)`; the fix itself is plan 05.)*
    `format!("modit-{}-2", esp_hal::chip!())` ([`buttons/src/main.rs:85`](../crates/buttons/src/main.rs#L85))
    — a manual disambiguator that has to be edited and reflashed per board.

27. ~~**[L] Naming drift.**~~ *(partially — the firmware callbacks `wf2`/`rf3` are now `write_led`/`read_button`. Renaming the `buttons` crate is an open decision deferred to plan 05.)* The crate is `buttons` but the module is a button *and* an
    LED. The RON type name is `Buttons(…)` for what `shared` calls a `Writable`.

28. **[L] No tests, no CI, no root `rust-toolchain.toml`, no `.env.example`.**

## Shape of the fix

The findings cluster into the three goals stated for this pass:

- **Straightforward to get into** → ~~17~~, ~~18~~, ~~19~~, 20, 21, 22, ~~23~~, ~~25~~, 28
- **Fail-free run** → 1, 2, ~~3~~, ~~4~~, 5, ~~11~~, ~~14~~
- **Good error messages** → ~~11~~, ~~12~~, ~~13~~, ~~14~~, ~~15~~, ~~16~~ — **all resolved**

See [`plans/README.md`](README.md) for the ordered work and current state.

## What is left, in one line each

The two highest-value remaining items, both in `brain`:

- **Finding 1** — `is_connected()` discards its result, so clean disconnects are
  invisible. One line, and plan 04 depends on it.
- **Finding 2** — the notification stream is reopened every 200 ms, so presses in
  the gap are lost. Structural, and the core of plan 04.
