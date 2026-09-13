//! Firmware for a modit coin acceptor module (ESP32 + a CH-92x-family selector).
//!
//! Unlike `module-button`, this one speaks the **message protocol** in
//! [`shared::proto`] rather than raw bytes: one service, two characteristics,
//! postcard-encoded [`Command`] in and [`Event`] out.
//!
//! | Characteristic | Access | Payload |
//! | -------------- | ------ | ------- |
//! | `command_charac` | write  | postcard [`Command`] |
//! | `event_charac`   | notify | postcard [`Event`] |
//!
//! # What a coin acceptor actually emits
//!
//! The acceptor does not report a value. It emits a **burst of pulses** on one
//! wire, and the number of pulses in the burst is how it names the coin it just
//! recognised -- taught by the programming button on the unit itself. So the
//! firmware's whole job is: count the pulses in a burst, decide the burst has
//! ended, and report the count as [`Event::Coin`]. What a pulse is *worth* is
//! venue policy and lives in the scenario, not here.
//!
//! # Why the pulse line is an interrupt and the button was not
//!
//! `module-button` samples its pin once per pass of the BLE work loop. That is
//! fine for a button: a press lasts long enough that no plausible loop overrun
//! can miss it, and a missed press is merely annoying.
//!
//! It is **not** fine here. A pulse can be as short as ~30 ms, and a missed
//! pulse does not lose an event -- it changes the *value* of the burst, turning
//! a two-credit coin into a one-credit coin. So the pulse line is an edge
//! interrupt with its own debounce, and the main loop only ever reads a count
//! that the ISR has already committed to.
//!
//! # Wiring
//!
//! **The acceptor runs on 12 V and the ESP32 does not.** Read
//! `docs/hardware/module-coin.md` before connecting anything, and measure every
//! wire against `GND` first -- clone units recolour them freely, and 12 V on a
//! GPIO destroys the pin.
//!
//! **GPIO27** is the COIN pulse line. The acceptor's output is open-collector,
//! so the internal pull-up sets the idle level: it idles high and each pulse
//! pulls it low. 12 V never reaches this wire.
//!
//! # Flashing
//!
//! `just flash-coin <id>` from the repository root.

#![no_std]
#![no_main]

#[macro_use]
extern crate alloc;

use core::cell::RefCell;

use alloc::collections::VecDeque;
use bleps::{
    ad_structure::{
        create_advertising_data, AdStructure, BR_EDR_NOT_SUPPORTED, LE_GENERAL_DISCOVERABLE,
    },
    attribute_server::{AttributeServer, NotificationData, WorkResult},
    gatt, Ble, HciConnector,
};
use critical_section::Mutex;
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    gpio::{Event as GpioEvent, Input, InputConfig, Io, Pull},
    handler, main,
    rng::Rng,
    time,
    timer::timg::TimerGroup,
};
use esp_println::println;
use esp_wifi::{ble::controller::BleConnector, init};
use shared::input::{saturating_u8, EdgeLatch};
use shared::proto::{self, Command, Descriptor, Event, Role, PROTOCOL_VERSION};

esp_bootloader_esp_idf::esp_app_desc!();

/// What kind of module this firmware is. Part of the advertised name.
const MODIT_ROLE: &str = "coin";

/// Shortest gap between two edges that can be two genuine pulses.
///
/// Anything faster is contact bounce or noise coupled in from the acceptor's
/// solenoid, which draws a 350 mA spike when it fires. Well below the ~30 ms
/// minimum pulse the acceptor is capable of emitting, so it cannot swallow a
/// real one.
const PULSE_DEBOUNCE_MS: u32 = 8;

/// Quiet time that marks the end of one coin's burst.
///
/// Chosen from the two numbers on the acceptor's datasheet:
///
/// - Pulses *within* one burst are at most ~100 ms apart, even on the slowest
///   pulse-width setting, so this must be comfortably above that.
/// - Recognition takes up to 0.6 s per coin, so two coins cannot produce bursts
///   closer together than that, and this must be comfortably below it.
///
/// 300 ms sits between the two with room on both sides. If you change the
/// acceptor's pulse-speed switch, re-check it against this.
const BURST_GAP_MS: u32 = 300;

/// Which physical board this is, baked in at flash time.
///
/// See the identical constant in `module-button` for why this is mandatory.
const MODIT_ID: &str = match option_env!("MODIT_ID") {
    Some(id) => id,
    None => panic!(
        "MODIT_ID is not set, so this board would be indistinguishable from every \
         other one. Flash with `just flash-coin <id>` from the repository root, for \
         example `just flash-coin slot`."
    ),
};

/// What this module tells the brain it is.
const DESCRIPTOR: Descriptor = Descriptor {
    protocol: PROTOCOL_VERSION,
    role: Role::Coin,
    // One input: the pulse line.
    inputs: 1,
    outputs: 0,
};

// The `gatt!` macro parses UUIDs at expansion time and so needs literals; these
// must stay in step with `shared::uuids`. Unlike `module-button`, which is
// policed by a string-matching test on the host, this crate can check itself:
// `str_eq` is `const`, so a drift is a compile error right here.
//
// They are `allow(dead_code)` because a `const _: () = assert!(..)` does not
// count as a use: their entire job is to be compared below.
#[allow(dead_code)]
const SERVICE_UUID: &str = "937312e0-2354-11eb-9f10-fbc30a62cf30";
#[allow(dead_code)]
const COMMAND_UUID: &str = "907312e0-2354-11eb-9f10-fbc30a62cf30";
#[allow(dead_code)]
const EVENT_UUID: &str = "8f7312e0-2354-11eb-9f10-fbc30a62cf30";
const _: () = assert!(
    shared::str_eq(SERVICE_UUID, shared::uuids::SERVICE),
    "SERVICE_UUID has drifted from shared::uuids::SERVICE"
);
const _: () = assert!(
    shared::str_eq(COMMAND_UUID, shared::uuids::COMMAND_WRITE),
    "COMMAND_UUID has drifted from shared::uuids::COMMAND_WRITE"
);
const _: () = assert!(
    shared::str_eq(EVENT_UUID, shared::uuids::EVENT_NOTIFY),
    "EVENT_UUID has drifted from shared::uuids::EVENT_NOTIFY"
);

/// The pulse pin, so the interrupt handler can clear its own interrupt.
static COIN_PIN: Mutex<RefCell<Option<Input<'static>>>> = Mutex::new(RefCell::new(None));

/// Pulses counted but not yet reported.
///
/// The counting, debouncing and burst-settling rules live in
/// [`shared::input`], where they are host-tested. This module supplies only the
/// clock and the pin.
static PULSES: EdgeLatch = EdgeLatch::new();

/// Milliseconds since boot, truncated to 32 bits.
///
/// Truncated because the Xtensa core has no 64-bit atomics, so a timestamp
/// shared with an interrupt handler cannot be wider than this. It wraps after
/// ~49 days; [`EdgeLatch`] compares with `wrapping_sub` throughout, so a wrap
/// costs at most one mis-timed burst boundary rather than a hang.
fn now_ms() -> u32 {
    time::Instant::now().duration_since_epoch().as_millis() as u32
}

/// Count one pulse from the acceptor.
///
/// Deliberately tiny: read the clock, debounce, bump a counter, clear the
/// interrupt. Everything that could block -- encoding, BLE, logging -- happens
/// in the main loop, because this runs with interrupts disabled while the radio
/// is mid-transaction.
#[handler]
fn coin_pulse() {
    PULSES.record(now_ms(), PULSE_DEBOUNCE_MS);

    critical_section::with(|cs| {
        if let Some(pin) = COIN_PIN.borrow_ref_mut(cs).as_mut() {
            pin.clear_interrupt();
        }
    });
}

/// Whether a full burst has arrived and gone quiet, and if so how big it was.
///
/// Returns `None` while a burst is still in progress, so a three-pulse coin is
/// reported once as `3` rather than three times as `1`.
fn finished_burst() -> Option<u8> {
    // `take_settled` holds back a burst that is still growing, and clears it
    // with a subtraction rather than a store, so a pulse arriving mid-read is
    // carried into the next coin instead of being lost. Both properties are
    // tested in `shared::input`; `saturating_u8` keeps an absurd burst visibly
    // absurd instead of wrapping 256 pulses into a free game.
    PULSES
        .take_settled(now_ms(), BURST_GAP_MS)
        .map(saturating_u8)
}

/// Called by `esp-backtrace` after it has printed the panic and backtrace.
///
/// Reboot rather than stall, for the reason spelled out in `module-button`.
#[unsafe(no_mangle)]
fn custom_halt() -> ! {
    println!("rebooting after panic");
    esp_hal::system::software_reset()
}

/// Report the pulse line's idle level.
///
/// The failure this catches is the expensive one: a COIN wire that idles low
/// is not the COIN wire, or has no shared ground, and every "pulse" the
/// firmware sees will be noise. Better to say so at boot than to hand out free credits.
fn self_test(coin: &Input<'_>) {
    // Open-collector output plus the internal pull-up: released idles high.
    if coin.is_low() {
        println!(
            "self-test: WARNING -- coin line (GPIO27) reads LOW at rest. Expected \
             high. Either the acceptor is not powered, its GND is not joined to \
             the ESP32's, its NO/NC switch is on NC, or this pin is not the COIN \
             wire. Do not trust the \
             pulse counts until this reads high."
        );
    } else {
        println!("self-test: coin line (GPIO27) idles high, as expected");
    }
    println!("self-test: post a coin now -- you should see a line per burst");
}

#[main]
fn main() -> ! {
    esp_println::logger::init_logger_from_env();

    println!(
        "modit {MODIT_ROLE} module, id {MODIT_ID:?}, on {}",
        esp_hal::chip!()
    );
    println!(
        "protocol v{PROTOCOL_VERSION}, burst gap {BURST_GAP_MS} ms, debounce {PULSE_DEBOUNCE_MS} ms"
    );

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::_80MHz);
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let esp_wifi_ctrl = init(
        timg0.timer0,
        Rng::new(peripherals.RNG),
        peripherals.RADIO_CLK,
    )
    .unwrap();

    // Pull-up, because the acceptor's COIN output is open-collector: it can pull
    // the line down but never drives it high.
    let coin_config = InputConfig::default().with_pull(Pull::Up);
    let mut coin = Input::new(peripherals.GPIO27, coin_config);

    self_test(&coin);

    // Route GPIO interrupts to `coin_pulse`, then arm the falling edge. Falling,
    // not any-edge: one pulse must count once, and the acceptor pulls the line
    // low for the pulse and releases it afterwards.
    let mut io = Io::new(peripherals.IO_MUX);
    io.set_interrupt_handler(coin_pulse);
    critical_section::with(|cs| {
        coin.listen(GpioEvent::FallingEdge);
        COIN_PIN.borrow_ref_mut(cs).replace(coin);
    });

    let mut bluetooth = peripherals.BT;
    let now = || time::Instant::now().duration_since_epoch().as_millis();

    loop {
        let connector = BleConnector::new(&esp_wifi_ctrl, bluetooth.reborrow());
        let hci = HciConnector::new(connector, now);
        let mut ble = Ble::new(&hci);

        println!("{:?}", ble.init());
        println!("{:?}", ble.cmd_set_le_advertising_parameters());
        println!(
            "{:?}",
            ble.cmd_set_le_advertising_data(
                create_advertising_data(&[
                    AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
                    AdStructure::ServiceUuids16(&[bleps::att::Uuid::Uuid16(0x1809)]),
                    AdStructure::CompleteLocalName(&format!("modit-{MODIT_ROLE}-{MODIT_ID}")),
                ])
                .unwrap()
            )
        );
        println!("{:?}", ble.cmd_set_le_advertise_enable(true));
        println!("started advertising as modit-{MODIT_ROLE}-{MODIT_ID}");

        // Events waiting for a subscribed client. A `RefCell` rather than a
        // static: the write closure below and the main loop both borrow it, but
        // both live on this one thread, so shared borrows of a `RefCell` are
        // enough and there is no interrupt in the path.
        let outbox: RefCell<VecDeque<Event>> = RefCell::new(VecDeque::new());

        // Greet on connect, unprompted, exactly as the protocol specifies.
        outbox.borrow_mut().push_back(Event::Hello(DESCRIPTOR));

        let mut write_command = |offset: usize, data: &[u8]| {
            match proto::decode::<Command>(data) {
                Ok(Command::SetOutput { channel, .. }) => {
                    println!("ignoring SetOutput on channel {channel}: this module has no outputs");
                }
                Ok(Command::Reset) => {
                    // Anything counted but unreported belongs to the previous
                    // session and must not be credited to the next player.
                    let dropped = PULSES.discard();
                    if dropped > 0 {
                        println!("reset: discarding {dropped} uncounted pulse(s)");
                    }
                }
                Ok(Command::Describe) => outbox.borrow_mut().push_back(Event::Hello(DESCRIPTOR)),
                Err(err) => {
                    println!("undecodable command at offset {offset}: {err} ({data:?})");
                }
            }
        };

        // Reading the event characteristic is not part of the protocol; the
        // brain subscribes. Report zero bytes rather than inventing a payload.
        let mut read_event = |_offset: usize, _data: &mut [u8]| 0;

        gatt!([service {
            uuid: "937312e0-2354-11eb-9f10-fbc30a62cf30",
            characteristics: [
                characteristic {
                    name: "command_charac",
                    uuid: "907312e0-2354-11eb-9f10-fbc30a62cf30",
                    write: write_command,
                },
                characteristic {
                    name: "event_charac",
                    uuid: "8f7312e0-2354-11eb-9f10-fbc30a62cf30",
                    notify: true,
                    read: read_event,
                },
            ],
        },]);

        let mut rng = bleps::no_rng::NoRng;
        let mut srv = AttributeServer::new(&mut ble, &mut gatt_attributes, &mut rng);
        let mut buf = [0u8; proto::MAX_MESSAGE_LEN];

        loop {
            // A completed burst becomes one Coin event. Queued rather than sent
            // here, so the "is anyone listening" check happens in one place.
            if let Some(pulses) = finished_burst() {
                println!("coin: {pulses} pulse(s)");
                outbox
                    .borrow_mut()
                    .push_back(Event::Coin { channel: 0, pulses });
            }

            let mut cccd = [0u8; 1];
            let subscribed = srv
                .get_characteristic_value(event_charac_notify_enable_handle, 0, &mut cccd)
                .is_some()
                && cccd[0] == 1;

            // Note the borrow is released before `do_work_with_notification`
            // below: this crate is edition 2021, so a `let` chain here would not
            // compile, and holding a `RefCell` borrow across the BLE call would
            // be a latent panic anyway.
            let queued = if subscribed {
                outbox.borrow_mut().pop_front()
            } else {
                None
            };
            let mut notification = None;
            if let Some(event) = queued {
                match proto::encode(&event, &mut buf) {
                    Ok(bytes) => {
                        notification = Some(NotificationData::new(event_charac_handle, bytes))
                    }
                    // Unreachable for the variants this module sends -- there is
                    // a test in `shared` asserting every one fits. Drop rather
                    // than retry, so a bug cannot wedge the loop forever.
                    Err(err) => println!("dropping {event:?}, cannot encode: {err}"),
                }
            }

            match srv.do_work_with_notification(notification) {
                Ok(WorkResult::GotDisconnected) => {
                    println!("client disconnected, re-advertising");
                    break;
                }
                Ok(_) => {}
                Err(err) => println!("{err:?}"),
            }
        }
    }
}
