# 10 — Earn the battery life BLE was chosen for

**Audit:** new (2026-08-27) · **Goal:** the modules run on batteries · **Size:** M

## Why

BLE was chosen over WiFi because modules should run on batteries. That reasoning
is directionally right — BLE uses meaningfully less energy per bit, and far less
when idle.

But the firmware never sleeps, so almost none of that advantage is being
collected. The CPU runs flat out at 80 MHz for as long as the board is powered,
and that dominates the radio difference. Picking BLE for power and then busy-
waiting is like buying a hybrid and leaving the engine running on the drive.

**This plan is measurement-first on purpose.** The correct outcome may well be
"do nothing" — see step 1.

## Current state

`crates/module-button/src/main.rs`:

- The inner loop calls `button.is_high()` and `srv.do_work_with_notification()`
  as fast as the CPU allows. There is no sleep, no delay, no wait. The only
  `delay_millis` calls in the file are in the boot self-test.
- `CpuClock::_80MHz` is set, down from the 240 MHz default. This is already a good
  power decision and the only one currently in place.
- Advertising uses `cmd_set_le_advertising_parameters()` — the no-argument
  version, i.e. whatever bleps defaults to.
- `println!` runs on every press and on every BLE error. UART is not free.

Rough figures, to be replaced by measurements in step 1:

| State | Ballpark |
| ----- | -------- |
| ESP32 awake @ 80 MHz, radio idle | tens of mA |
| BLE connected with modem sleep | ~10–20 mA avg |
| Light sleep | ~1 mA |
| Deep sleep | ~10 µA |
| BLE TX peak | ~130 mA |
| WiFi TX peak | ~200 mA+ |

## What the stack actually allows

Checked against the pinned versions, because two of these are not what I assumed:

| Lever | Available? | How |
| ----- | ---------- | --- |
| CPU clock | ✅ already done | `CpuClock::_80MHz` |
| **Advertising** interval | ✅ yes | `cmd_set_le_advertising_parameters_custom` takes `AdvertisingParameters { advertising_interval_min, advertising_interval_max, .. }` |
| Idling the CPU instead of spinning | ✅ yes | bleps ships `async_attribute_server.rs`, and the `async` feature is **already enabled** in `Cargo.toml` |
| Manual light sleep | ⚠️ awkward | `esp_hal::rtc_cntl::Rtc::sleep_light(&mut self, wake_sources)` exists, but `WorkResult` is only `DidWork | GotDisconnected` — the sync server never says "nothing due until T", so there is nothing to schedule a sleep against |
| **Connection** interval | ❌ **not reachable** | bleps exposes no connection-parameter-update; and a central cannot set it either (btleplug has no API, and CoreBluetooth does not allow it) |
| Deep sleep between presses | ❌ not for this module | see below |

### Correction to earlier advice

I previously described the BLE **connection interval** as a power/latency dial you
could turn per module. **With this stack you cannot.** bleps has no connection
parameter update request, and the central side cannot set it either. The dial
exists in the BLE spec; it is not exposed here. Reaching it would mean patching
bleps or moving to another BLE stack — out of scope for this plan.

The **advertising** interval is genuinely controllable, and matters whenever a
module is powered but not yet connected.

### Why deep sleep does not apply

A button-only sensor could deep-sleep at ~10 µA and wake on a GPIO edge — months
on a coin cell. This module also has an LED the brain commands, so it must stay
*reachable*, which keeps the radio and CPU up.

That constraint comes from being bidirectional, not from BLE. It would apply just
as much over WiFi or ESP-NOW. Worth remembering when designing future module
types: **a pure notifier can be radically lower-power than an actuator**, and
splitting a prop into a sleepy sensor plus a mains-powered actuator is a real
design option.

## Steps

### 1. Measure, then decide whether to continue

- [ ] Put a USB power meter inline with one board and record the average current
      in three states: advertising but unconnected, connected and idle, connected
      with presses.
- [ ] Do the arithmetic for the battery you intend to use. As an anchor: 2000 mAh
      at 40 mA is about 50 hours.
- [ ] Write the number and the decision in the Notes below.

**If a day of sessions with an overnight charge is the real requirement, the
current firmware may already meet it — in which case stop here and record that.**
That is a successful outcome for this plan, not a failure.

### 2. Cheap wins, if step 1 says continue

- [ ] Lengthen the advertising interval with
      `cmd_set_le_advertising_parameters_custom`. A module waiting to be found may
      spend a long time in this state. Trade-off: longer interval means the brain
      takes longer to discover it, so re-check the acquisition timing in `brain`.
- [ ] Drop debug `println!` on hot paths, or put them behind a feature. UART
      costs current, and the per-press logging is the sort that gets left in.
- [ ] Try `CpuClock::_40MHz` and re-measure. BLE has modest compute needs; if the
      board keeps up, that is free.

### 3. The structural one: stop spinning

- [ ] Move to bleps' **async** attribute server (`async_attribute_server.rs`). The
      `async` feature is already enabled — this is a rewrite of the loop, not a
      dependency change.
- [ ] With an executor in place the CPU can idle between events instead of
      polling. Measure again: this is expected to be the largest single win, and
      it is what makes any later light-sleep work possible.
- [ ] The button then wants to be an interrupt/GPIO event rather than a polled
      `is_high()`, which also removes the debounce loop's dependence on loop
      timing entirely.

### 4. Only if measurement demands it

- [ ] Different silicon. The ESP32 is WiFi-first with BLE bolted on; an nRF52840
      is roughly an order of magnitude better at low-power BLE and has the
      strongest embedded-Rust BLE story (`embassy-nrf`). An ESP32-C3/C6 is a
      cheaper middle step that stays inside esp-rs.
- [ ] **Do not start here.** It is a firmware rewrite, and steps 1–3 are cheaper
      and may make it unnecessary.

## Done when

- [ ] A measured average current for each of the three states is written down.
- [ ] Estimated battery life for the intended pack is written down.
- [ ] Either the numbers meet the requirement (record it and close this plan), or
      steps 2–3 are done and the numbers are re-measured.
- [ ] The README's power claim matches what the firmware actually does.

## Notes

_(record the measurements here — they are the point of this plan)_
