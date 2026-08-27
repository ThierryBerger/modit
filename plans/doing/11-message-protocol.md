# 11 — Put the protocol in `shared`, not in the UUIDs

**Audit:** new (2026-08-27) · **Goal:** the project scales past one module type
· **Size:** L · **Do before or with [plan 09](09-scenario-seam.md)**

## Why

Today a module type *is* a set of UUIDs. Adding a module type means minting new
UUIDs, hardcoding them in the firmware's `gatt!` block, hardcoding a matching
definition in `brain`, and keeping the two in step by hand.

Three things follow from that, and they are all load-bearing:

1. **`shared` is not carrying its weight.** It is two structs of strings plus
   three constants. The protocol — what a module can be told, what it can
   report — exists only as an informal agreement between two files.
2. **It cannot express a prop with more than one input or output.** `ButtonLed`
   is exactly one `Notifier` plus one `Writable`. A prop with three buttons and a
   seven-segment display has no representation at all. Escape games are full of
   those.
3. **The drift test exists to paper over the design.**
   `firmware_uuids_match_shared` reads the firmware's source with string matching
   because there is no shared definition to compare against. It works, and it
   should not need to exist.

Replacing UUID-as-protocol with messages-as-protocol removes all three, and makes
`--simulate` and a future transport swap nearly free.

## Current state

- `crates/shared/src/lib.rs` — `Notifier`, `Writable`, three UUID constants.
- `crates/module-button/src/main.rs` — a `gatt!` block with one service and two
  characteristics, UUIDs as string literals (the macro parses them at expansion
  time and cannot take a `const`).
- `crates/brain/src/ble/mod.rs` — `ModuleDefinition` finds a characteristic by
  UUID; `Module` adds `advertised_name()`.
- `crates/brain/src/main.rs` — `ButtonLed` hardcodes which UUIDs mean what.
- Payloads today are `&[0]` / `&[1]` for the LED and the literal bytes
  `b"Notification"` for a press. There is no encoding, only convention.

## Target state

**One service. Two characteristics. Forever.**

```
service MODIT_SERVICE
  characteristic COMMAND   write   <- postcard(Command)
  characteristic EVENT     notify  -> postcard(Event)
```

The protocol moves into `shared`:

```rust
pub const PROTOCOL_VERSION: u8 = 1;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Drive an output. `channel` indexes the module's outputs.
    SetOutput { channel: u8, on: bool },
    /// Return to the state a module has at boot.
    Reset,
    /// Ask the module to re-send its `Hello`.
    Describe,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Sent unprompted on connect, and in reply to `Describe`.
    Hello(Descriptor),
    Pressed { channel: u8 },
    Released { channel: u8 },
    Measurement { channel: u8, value: i32 },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Descriptor {
    pub protocol: u8,   // PROTOCOL_VERSION at build time
    pub role: Role,
    pub inputs: u8,
    pub outputs: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role { Button, /* Nfc, Motor, Screen, ... */ }
```

Serialised with **postcard** — `no_std`, no alloc, serde-based, and compact
(varint encoding makes these 2–5 bytes each). `shared` already depends on serde.

### What this buys

- **Adding a module type is adding enum variants.** No new UUIDs, ever.
- **Multi-channel props work**: `SetOutput { channel: 2, .. }`.
- **Capability discovery.** A module announces itself; `brain` checks the
  scenario against what the board says it is, instead of inferring from which
  characteristics happen to exist.
- **Version mismatches become explicit.** `Descriptor::protocol` replaces today's
  guess — *"advertises the right name but does not expose the characteristics
  module 0 needs — stale firmware?"* — with a message naming both versions.
- **`--simulate` becomes trivial:** a `Link` impl over an in-process channel
  carrying the same enums. This is the single biggest re-entry win in the backlog.
- **A transport swap becomes a trait impl**, not a rewrite. ESP-NOW or MQTT later
  is then a decision, not a migration.

### What this deletes

- `firmware_uuids_match_shared` and the string-matching that powers it. Three
  UUID constants remain and never change again.
- `Notifier`, `Writable`, and most of `ModuleDefinition`.
- The `&[0]`/`&[1]` and `b"Notification"` conventions.

## Constraints, checked against the pinned versions

| Constraint | Detail |
| ---------- | ------ |
| **ATT MTU** | bleps defaults to `BASE_MTU = 23`, i.e. **20 usable payload bytes**. It *does* handle MTU exchange, and offers `mtu128` / `mtu256` cargo features — **neither is currently enabled.** |
| `gatt!` needs literals | Unchanged, but it stops mattering: three literals defined once, never touched again. |
| btleplug MTU | The central side does not expose MTU control. On macOS CoreBluetooth negotiates automatically. **Verify the negotiated size on hardware before relying on more than 20 bytes.** |
| Notification payload | `NotificationData::new(handle, &[u8])` — a plain slice, so any encoding is fine. |

**Design every message to fit in 20 bytes.** `Command` and `Event` variants above
encode to a handful of bytes each. `Hello(Descriptor)` is the one at risk if
`Role` or `Descriptor` grows — keep it small, and enable `mtu256` as insurance
rather than as a dependency.

## Steps

Staged so that each step is independently verifiable, and step 1 needs no
hardware at all.

### 1. Define the protocol (no hardware, breaks nothing)

- [ ] Add `postcard` to `shared` (`no_std`, no alloc).
- [ ] Add `shared::proto` with `Command`, `Event`, `Descriptor`, `Role`,
      `PROTOCOL_VERSION`.
- [ ] Add `encode`/`decode` helpers over fixed-size buffers, returning a real
      error rather than panicking on a short buffer.
- [ ] **Round-trip tests for every variant**, plus an assertion that each
      encodes to **≤ 20 bytes**. That assertion is the design constraint made
      executable; it will catch the day someone adds a `String` to `Descriptor`.
- [ ] Leave everything else untouched. This step is a pure addition.

### 2. Firmware speaks both protocols

- [ ] Add the `COMMAND` and `EVENT` characteristics **alongside** the existing
      ones. Both work at once.
- [ ] Send `Hello(Descriptor)` on connect.
- [ ] Enable `mtu256` on bleps, and log the negotiated MTU at boot so the real
      number is known rather than assumed.
- [ ] Flash and verify with a phone BLE scanner — write a `SetOutput`, watch a
      `Pressed` arrive. **The old path still works, so nothing is lost if this
      step is wrong.**

### 3. Brain switches over

- [ ] Add a `Link` trait: `send(ModuleId, Command)` and an event stream of
      `(ModuleId, Event)`. This is the transport seam — see plan 09.
- [ ] Implement it for BLE over the two new characteristics.
- [ ] Move the scenario onto `Event::Pressed` / `Command::SetOutput`.
- [ ] Check `Descriptor::protocol` on connect and refuse a mismatch with a
      message naming both versions.
- [ ] Check the scenario's expectations against the `Descriptor` — a scenario
      wanting output channel 2 from a module reporting `outputs: 1` should fail
      loudly at bind time, not silently at runtime.

### 4. Delete the old path

- [ ] Remove the old characteristics from the firmware.
- [ ] Remove `Notifier`, `Writable`, and the UUID drift test.
- [ ] Collapse `ModuleDefinition` / `Module` into whatever plan 09's seam needs.

### 5. Collect the winnings

- [ ] Implement `Link` for an in-process channel — this is `--simulate`.
- [ ] Give the tutorial a step 0 that runs a scenario with no hardware.

## Done when

- [ ] Adding a hypothetical `Role::Motor` requires touching **no UUIDs** and no
      firmware other than the new module's own.
- [ ] A scenario can address a prop with more than one input and more than one
      output.
- [ ] A module running mismatched firmware is rejected with a message naming both
      protocol versions.
- [ ] `--simulate` runs a full round with no radio.
- [ ] `grep -rn "firmware_uuids_match_shared"` finds nothing.

## Notes

### Sequencing with plan 09

These two overlap: **plan 09's `Link`/runtime seam and this plan's message
protocol are the same boundary seen from two sides.** Doing 09 first and then
retrofitting messages means designing the scenario API against `Pressed`-shaped
calls and then generalising it. Doing them together is barely more work than 09
alone.

Concretely, plan 09's primitive changes from

```rust
async fn next_press(&self, timeout: Duration) -> Result<ModuleId, WaitError>;
```

to

```rust
async fn next_event(&self, timeout: Duration) -> Result<(ModuleId, Event), WaitError>;
```

with `next_press` as a filter over it. Same argument as the one that made
`next_press` beat `wait_for_press`: build the general primitive first, because the
specific one cannot be widened later without rewriting every scenario.

### Why not do this before the hardware verification?

Steps 2–4 change the firmware, and there are already three plans in `doing/`
waiting on a board. **Step 1 is safe to do at any time** — it is a pure addition
with host-only tests. Steps 2 onward should wait until `CHECKME.md` is clear, so a
misbehaving board can be attributed to one change rather than two.

### Deliberately not decided here

- **Whether `Role` belongs in `shared` at all**, versus modules being described
  purely by their channel counts. A generic descriptor is more flexible; a `Role`
  enum gives better error messages. Start with `Role`; it is easy to relax.
- **Acknowledgements.** `SetOutput` is fire-and-forget today. If a lost LED
  command ever matters, that wants a sequence number, which wants a design.
  Not now.
