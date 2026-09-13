# 13 — A brainless mode: modules pass a token between themselves

**Audit:** new (2026-09-02) · **Goal:** a game with no laptop in it · **Size:** L
· **Independent of 04/06/11** -- deliberately additive, deletes nothing

## Why

Every scenario today needs a laptop in the room. The brain scans, connects,
holds the game state, and if it walks out the door the props are furniture. For
an escape game that is a real constraint: a machine to set up, a BLE stack whose
connection cap is single digits, and a single point of failure with a battery
and a screen.

The alternative is that the modules coordinate among themselves. No central, no
connections, no laptop -- power on three boxes and a game exists.

This plan builds that as an **isolated experiment**. It does not touch `brain`,
`shared::proto`, or the two working firmwares. If it turns out to be a bad idea
it is one `rm -r` away from never having happened.

### Why a token, and not whack-a-mole

The obvious first game is whack-a-mole, and the obvious first instinct is to
port `scenarios::whack_a_mole` onto a module. That instinct is wrong, and it is
worth writing down why.

Whack-a-mole is defined by a *schedule*: something decides which mole lights and
when. Without a brain, that something has to be elected, and now the interesting
part of the system is a leader election protocol whose failure mode is the game
stopping.

Token passing has no schedule. **The lit module is whoever holds the token**;
pressing it hands the token on. To a player this is whack-a-mole -- one box is
lit, hit it, another lights up. But the state is one value instead of a plan, so
a module that misses an update, reboots, or walks out of range can rejoin by
adopting what it hears. Disconnection tolerance is not a feature to add; it is
what is left when the schedule is removed.

The dynamic module count falls out of the same place: the token holder picks its
successor from whoever it has heard from recently. Nobody needs to agree on how
many boxes are playing.

## Current state

- `crates/brain` owns all game logic and all module state. Modules are passive
  GATT peripherals -- `crates/module-button/src/main.rs` only reacts to writes
  and notifies on a press.
- The BLE stack is `bleps` (pinned at rev `a5148d8`), which is a **peripheral-only
  GATT server**. Checked directly against the vendored source:
  - its commands stop at `cmd_set_le_scan_rsp_data` (the peripheral answering a
    scan request); there is no `LE Set Scan Parameters` and no `LE Set Scan Enable`;
  - `event::EventType` has no advertising-report variant, so a scan result would
    parse as `Unknown`.

  **A module therefore cannot currently hear another module.** That is the one
  real blocker, and step 1 removes it.
- Modules have a stable identity already (`MODIT_ID`, baked in at flash time --
  plan 05), which this plan reuses.

## Target state

Three or more boards, no laptop. Exactly one is lit. Press it and another lights.
Power a fourth on mid-game and it joins within a second. Pull the battery out of
the lit one and the game recovers by itself. Plug it back in and it rejoins.

### The transport: an advertising bus

No connections at all. Every module simultaneously:

- **advertises** its current view of the world, continuously, and
- **passively scans** for everyone else's.

This needs no BLE host library. `esp_wifi::ble::controller::BleConnector` is a
raw HCI byte pipe (`embedded_io::Read`/`Write`); `bleps` is merely one consumer
of it. The bus needs six HCI commands and one event:

| Direction | HCI |
| --------- | --- |
| → | `Reset`, `LE Set Advertising Parameters`, `LE Set Advertising Data`, `LE Set Advertise Enable` |
| → | `LE Set Scan Parameters`, `LE Set Scan Enable` |
| ← | `LE Advertising Report` |

`bt-hci 0.3.2` already has all of these (`cmd::le::LeSetScanParams`,
`LeSetScanEnable`, `event::le::LeAdvertisingReport`) **and is already in the
dependency tree** -- esp-wifi pulls it, and `BleConnector` implements
`bt_hci::transport::Transport`. So this is a new consumer of an existing
dependency, not a stack migration. `trouble-host` is explicitly *not* needed:
it would bring GATT, ATT and L2CAP to carry eleven bytes.

Two settings are load-bearing and easy to get wrong:

- **`filter_duplicates = 0`** on `LE Set Scan Enable`. With filtering on, the
  controller suppresses repeat advertisements from an address -- which is
  precisely the traffic this design is made of.
- **Passive scanning.** Everything lives in the ADV packet; there is no scan
  response, so active scanning would only add scan-request noise.

### The payload

A legacy advertising PDU carries 31 bytes of AD data. Dropping the `Flags`
structure (optional for a non-discoverable beacon) and using one
Manufacturer Specific Data structure -- 1 length + 1 type `0xFF` + 2 company id,
using `0xFFFF`, the SIG's unregistered/test value -- leaves **27 payload bytes**.

The state is eleven:

```rust
#[repr(C)]
pub struct Beacon {
    magic:   u8,   // b'M' -- reject foreign advertisements for one byte
    version: u8,   // this bus's protocol version, unrelated to shared::proto
    sender:  u16,  // who is advertising (derived from MODIT_ID)
    term:    u32,  // monotonic; the total order over handoffs
    holder:  u16,  // who the sender believes holds the token
    flags:   u8,   // bit 0: holder reports its input active
}
```

Note the budget: **27 bytes against 11 used**, versus the 20-byte ceiling
`shared::proto` fights for in [plan 11](../doing/11-message-protocol.md). There
is room here for a round counter, a score, or a session id later. There is no
size pressure, so do not optimise for it.

`term` is `u32` rather than `u16` on purpose: at one handoff a second, `u16`
wraps in eighteen hours of play, and wrapping comparisons are a good way to
produce a bug that only appears at a real event.

### The rules

The entire protocol, and it is short enough to hold in your head:

1. **Advertise unconditionally**, every ~100 ms, whatever you currently believe.
2. **Adopt a strictly higher term.** Hearing `(term', holder')` with
   `term' > term` sets your own state to it. Equal terms tie-break on the lower
   `holder` id, so two simultaneous claims converge instead of oscillating.
3. **`lit = (holder == me)`.** The LED is a pure function of adopted state, never
   a received command. This is the line that makes reconnection free.
4. **Handoff.** When the holder's button is pressed, it picks a successor
   uniformly at random from its live peers (excluding itself, unless it is
   alone), then sets `term += 1, holder = successor`. Rule 1 does the rest.
5. **Liveness is the advertisement.** Keep a fixed-size table of
   `(id, last_heard_at)`. A peer is live if heard within `PEER_TIMEOUT` (~3 s).
   This is the "alive ping", and it costs no extra traffic.
6. **A lost token is a silent holder.** If nothing has been heard *from* the
   current `holder` for `TOKEN_TIMEOUT` (~1 s), wait a random backoff and, if
   still silent, **claim**: `term += 1, holder = me`. Randomised backoff plus
   rule 2's tie-break resolves a double claim in a couple of intervals.
7. **Listen before joining.** On boot, scan only for `WARMUP` (~500 ms) and adopt
   the highest term heard before advertising anything.

Rule 7 is what makes rejoining work. A rebooted module starts at `term = 0`; if
it advertised immediately it would be ignored forever by a swarm at term 5000,
and would meanwhile believe itself the holder. Warming up first is cheaper and
less surprising than persisting the term in NVS, and it is the same mechanism
that lets a brand-new board join a game already in progress.

### What this deliberately is not

- **Not connected to `brain`.** A board running this firmware is not a GATT
  peripheral and the brain cannot see it. Brainless mode and brain mode are
  different firmwares for now. Unifying them is a later decision, taken with
  evidence, not now.
- **Not using `shared::proto`.** The bus has its own wire format. `Command`/`Event`
  are designed for a point-to-point link with a central; this is a broadcast
  state bus. Forcing one onto the other before either has run would be guessing.
- **Not a mesh in the routing sense.** Single hop, everyone in radio range of
  everyone. Multi-hop relaying is a different plan and probably a different
  protocol.

## Steps

Ordered so the riskiest unknown is settled first, and so the game logic is
correct on a laptop before it ever meets a radio.

### 1. Prove the radio can do it (hardware, ~an afternoon)

**This is the step that can kill the plan, so it goes first.** The design assumes
an ESP32 can advertise and scan *at the same time*.

- [ ] New firmware crate `crates/module-token` (own workspace, like the other two).
- [ ] Talk HCI to `BleConnector` directly via `bt-hci`. No `bleps`.
- [ ] Advertise a fixed 11-byte payload and passively scan with
      `filter_duplicates = 0`; print every report that starts with the magic byte.
- [ ] Flash two boards. **Each must see the other, continuously, for minutes.**
- [ ] Measure and write down here: reports/second per peer, and the observed
      latency between changing the advertising data and a peer seeing it.
- [ ] Check that `LE Set Advertising Data` while advertising is enabled swaps
      cleanly, since every handoff does exactly that.

**Measure the advertising interval carefully.** BT 4.x sets a 100 ms floor for
non-connectable undirected advertising, and the ESP32's controller is 4.2. If
100 ms makes the LED feel laggy, the fix is to advertise as `ADV_IND`
(connectable, 20 ms floor) and simply never accept a connection. Decide with the
number, not in advance.

If advertising and scanning cannot genuinely run concurrently, the fallback is
software duty-cycling -- alternate advertise and scan windows -- which works but
adds latency. **Find this out before writing a state machine on top of it.**

### 2. The state machine, with no I/O at all (host, no hardware)

- [ ] New crate `crates/mesh` in the host workspace: `no_std`, no alloc, no I/O.
- [ ] `Beacon` encode/decode with round-trip tests, as `shared::proto` does it.
- [ ] A pure `Node::step(now_ms, heard: &[Beacon], input: Input) -> Outcome`
      returning the beacon to advertise and whether the LED is on. **No timers,
      no radio, no async.** Time is an argument.
- [ ] Fixed-capacity peer table (say 16), so no allocator is involved.

Keeping this pure is the highest-value decision in the plan: it is the difference
between debugging a distributed protocol by reflashing three boards and debugging
it in `cargo test`.

### 3. A swarm simulator (host)

- [ ] `crates/mesh/examples/swarm.rs`: N nodes over a simulated bus, seeded and
      deterministic, with configurable loss rate, latency and clock skew.
- [ ] Tests, and these are the acceptance criteria in executable form:
  - [ ] From cold boot, exactly one holder within a bounded time.
  - [ ] After a handoff, exactly one holder.
  - [ ] Kill the holder -> exactly one holder again within `TOKEN_TIMEOUT` + backoff.
  - [ ] **Never two holders once converged**, at 30% packet loss.
  - [ ] A node joining at term 0 adopts the swarm's term and never disrupts it.
  - [ ] Partition the swarm in two, let both halves claim, heal it -> converges
        to one holder. (Two holders *during* a partition is correct and expected;
        say so in the test name.)
- [ ] `just swarm` to watch it run.

### 4. Put step 2 on step 1's radio

- [ ] `module-token` drives `mesh::Node` from the real button, LED and clock.
- [ ] Serial log every adopted state change: `term`, `holder`, why.
- [ ] Flash three boards and play it.

### 5. The things only hardware can answer

- [ ] Power a fourth board on mid-game; time how long until it can receive the token.
- [ ] Pull power from the lit board; time the recovery.
- [ ] Restore it; confirm it rejoins and can be lit again.
- [ ] Walk one out of range and back.
- [ ] Measure end-to-end press-to-light latency and record it below. This is the
      number that decides whether the mode is fun.
- [ ] Note the current draw against [plan 10](10-power-and-battery.md) --
      continuous scanning is not free, and this is the first firmware here that
      does it.

### 6. Decide, in writing

- [ ] Whether it is fun. A ~200 ms light-up delay may be fine or may be fatal.
- [ ] Whether brain mode and brainless mode should converge (one firmware, a
      runtime switch) or stay separate builds.
- [ ] Whether `Beacon` should grow toward `shared::proto`, or stay its own thing.
- [ ] If the answer to the first question is no: delete `crates/mesh` and
      `crates/module-token`, and keep this file. That is a successful experiment.

## Done when

- [ ] Three boards, powered from batteries, with **no laptop in the room**, play a
      recognisable game of whack-a-mole.
- [ ] A board powered on mid-game joins and takes its turn, with no restart.
- [ ] A board losing power mid-turn does not end the game; the token recovers.
- [ ] That same board, powered back on, rejoins.
- [ ] `cargo test` covers loss, partition, reboot and join without any hardware.
- [ ] Press-to-light latency is measured and written down in the Notes below.
- [ ] `crates/brain`, `crates/shared`, `crates/module-button` and
      `crates/module-coin` are **unchanged** by this plan.

## Notes

### Why not ESP-NOW

It is the natural fit -- symmetric, connectionless, 250-byte payloads -- but it
is the Wi-Fi PHY, so it abandons the power story the project is built on
([plan 10](10-power-and-battery.md)), and running it alongside BLE needs
esp-wifi's experimental `coex` feature. Staying on BLE keeps one radio, one
story, and one set of trade-offs. Reconsider only if step 1 shows the BLE
controller cannot advertise and scan concurrently.

### Why not elect a leader and reuse the brain's scenarios

It was the tempting option, because `whack_a_mole` would then be reused verbatim.
It was rejected because the leader is a new single point of failure that has to
be *detected* and *replaced*, and because the reuse is an illusion: the scenario
would still need rewriting to be `no_std`, alloc-free and generic over an async
trait. Token passing needs no election, and its recovery path (rule 6) is the
same code as its normal path.

The one thing lost is a central schedule. If the game feels aimless without one,
the cheap fix is one more timer: if the holder is not pressed within N seconds it
passes the token on anyway. That is three lines, not a leader.

### Deliberately not decided here

- **Scoring.** There is room in the payload for it, and no idea yet of what it
  should mean when a module can leave mid-game.
- **Multiple simultaneous tokens.** Two lit boxes is a different and possibly
  better game. The `term`/`holder` design assumes one; generalising is not free.
- **Security.** Anyone within range can advertise a higher term and take over the
  game. For an escape game in a controlled room this is fine. Say it out loud
  rather than discovering it.

### Measurements

Fill these in as they are taken -- these numbers are the point of steps 1 and 5.

| What | Expected | Measured |
| ---- | -------- | -------- |
| Advertising interval actually accepted | 100 ms (20 ms if `ADV_IND`) | |
| Reports received per peer per second | | |
| Press → successor lit | 100-300 ms | |
| Recovery after the holder dies | 1-2 s | |
| A new board joining → able to hold | ~1 s | |
| Current draw while advertising + scanning | | |
