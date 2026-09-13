# 12 — A coin acceptor module: the ESP32, and coins over BLE

**Audit:** new (2026-08-28), split 2026-09-13 · **Goal:** the project has a
second module type · **Size:** M · **Depends on [plan 11](11-message-protocol.md)
steps 2–3**

The finished half of plan 12 -- protocol, simulator, `arcade`, and an Arduino Uno
counting real coins -- is [`done/12-coin-module-uno.md`](../done/12-coin-module-uno.md).
Why a coin acceptor at all is argued there.

## Current state

**The acceptor side is settled.** The Uno bench confirmed the wires, the NO
setting, that `COIN` is open-collector, that 8 ms / 300 ms split bursts
correctly, and that the solenoid's spike does not upset a board sharing its
supply. See [what the bench established](../done/12-coin-module-uno.md#confirmed-on-the-bench-2026-09-13).
None of that needs re-proving on the ESP32.

**What is not settled is the ESP32 itself.** `crates/module-coin` compiles, links,
and uses the same `shared::input` and `shared::coin` rules the Uno does, but it has
never been flashed onto a board wired to the acceptor.

**And `arcade` runs under `--simulate` only.** `link/ble.rs` is typed to
`ButtonLed`/`ButtonDetails` and still translates the *old* wire format —
`&[0]`/`&[1]` out, `b"Notification"` in. A coin's pulse count cannot be expressed
in that format at all, so there is nowhere to put a coin module. `brain` says so
and refuses rather than scanning for a board it could not drive.

The Uno has no radio and is not part of this plan.

## Target state

A `modit-coin-<id>` ESP32 that reports `Event::Coin { channel, pulses }` over
BLE, and `just brain --scenario arcade` taking a real coin.

## Steps

### 4b. ESP32 on the acceptor (written, **needs the hardware**)

- [x] `crates/module-coin`, speaking `shared::proto` over the two new
      characteristics. Compiles and links; `just build-firmware` covers it.
- [x] Falling-edge interrupt with the `shared::coin` debounce and burst gap.
- [x] Boot self-test: report the pulse line's idle level.
- [x] Identify the acceptor's wires and set its NO/NC switch -- done on the Uno
      bench, and a property of the acceptor, not of the board.
- [ ] **Flash it and post a coin.** Nothing below this line has been run against
      real hardware.
- [ ] Confirm the pulse line idles high on the ESP32's internal pull-up alone,
      with `R1` (1 k) in series. The Uno had a 10 k external pull-up; a ~45 k
      internal one to 3.3 V is weaker, and is the one ESP32-specific doubt in the
      wiring.
- [ ] Confirm a coin taught as *n* pulses arrives as one `Coin { pulses: n }`,
      not as *n* separate events.
- [ ] Confirm the ESP32 does not brown out when the coil fires. Its supply is USB,
      not the acceptor's, so the Uno's result does not carry over; only the shared
      ground does.

### 5. Coin over BLE (**not started**)

This is exactly [plan 11](11-message-protocol.md) steps 2–3, and doing it needs
the button firmware moved onto `shared::proto` as well:

- [ ] Add `COMMAND`/`EVENT` to `module-button` alongside its existing
      characteristics (plan 11 step 2 — "both work at once", so nothing is lost
      if it is wrong).
- [ ] Make the BLE link speak `proto::encode`/`decode` over those two
      characteristics, for any module, with no per-role plumbing. It should get
      *smaller*: the role only affects the advertised name.
- [ ] Replace `Bench::button_modules` with something role-agnostic and delete the
      refusal in `main`.
- [ ] Plan 11 step 4: delete the old characteristics, `Notifier`, `Writable`, and
      `firmware_uuids_match_shared`.

## Done when

- [ ] `just brain --scenario arcade` takes a real coin and starts a real game.
- [ ] A coin taught as three pulses gives three credits, repeatably.
- [ ] `grep -rn "cannot drive one yet" crates/` finds nothing.

## Notes

### Deliberately not decided here

- **Whether `Event::Coin` should be a general `Counted { channel, count }`**,
  which would also fit a turnstile or a pulse water meter. `Coin` was chosen for
  the better log lines, on the same reasoning plan 11 used for keeping `Role`:
  start specific, relax later. Relaxing it is a rename.
- **Crediting across a disconnect.** Pulses counted but unreported are discarded
  on `Reset`, and lost outright if the board reboots. Making that safe means the
  module holding credit in NVS and the brain acknowledging it, which wants the
  sequence numbers plan 11 also deferred. A dropped coin is currently a lost
  coin; for a game that is annoying, and for a real cash box it would not be
  acceptable.
