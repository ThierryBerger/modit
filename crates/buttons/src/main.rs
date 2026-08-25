//! Firmware for a modit button+LED module (ESP32).
//!
//! Advertises over BLE and exposes a single GATT service with two
//! characteristics:
//!
//! | Characteristic | Access | Purpose |
//! | -------------- | ------ | ------- |
//! | `led_charac`    | write  | one byte: `0` turns the LED off, anything else on |
//! | `button_charac` | notify | fires once per button press, if the client subscribed |
//!
//! # Wiring
//!
//! - **GPIO33** -- button to 3V3. Configured with an internal pull-down, so the
//!   pin reads high while the button is held.
//! - **GPIO26** -- LED (through a current-limiting resistor) to GND.
//!
//! # Flashing
//!
//! `just flash` from the repository root. Requires the `esp` toolchain and
//! `espflash`; `just setup` installs both.

#![no_std]
#![no_main]

#[macro_use]
extern crate alloc;

use bleps::{
    ad_structure::{
        create_advertising_data, AdStructure, BR_EDR_NOT_SUPPORTED, LE_GENERAL_DISCOVERABLE,
    },
    attribute_server::{AttributeServer, NotificationData, WorkResult},
    gatt, Ble, HciConnector,
};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull},
    main,
    rng::Rng,
    time,
    timer::timg::TimerGroup,
};
use esp_println::println;
use esp_wifi::{ble::controller::BleConnector, init};

esp_bootloader_esp_idf::esp_app_desc!();

/// How long to ignore further edges after a button press, in milliseconds.
const DEBOUNCE_MS: u64 = 50;

/// Called by `esp-backtrace` after it has printed the panic message and
/// backtrace (via the `custom-halt` feature).
///
/// The default behaviour is to stall the cores and spin forever, which in the
/// field is indistinguishable from a flat battery. Rebooting instead means a
/// module that hits a bug comes back on its own, and the serial log still has
/// the backtrace explaining why.
#[unsafe(no_mangle)]
fn custom_halt() -> ! {
    println!("rebooting after panic");
    esp_hal::system::software_reset()
}

#[main]
fn main() -> ! {
    esp_println::logger::init_logger_from_env();

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

    let config = InputConfig::default().with_pull(Pull::Down);
    let button = Input::new(
        // external button
        peripherals.GPIO33,
        config,
    );

    let mut bluetooth = peripherals.BT;
    let mut led = Output::new(peripherals.GPIO26, Level::Low, OutputConfig::default());

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
                    // Fully qualified: the bare `Uuid` name here would resolve only
                    // via the `use` that the `gatt!` macro below expands into.
                    AdStructure::ServiceUuids16(&[bleps::att::Uuid::Uuid16(0x1809)]),
                    // TODO(plan-05): the trailing "-2" is a per-board disambiguator
                    // that has to be hand-edited and reflashed. Replace with a
                    // compile-time MODIT_ID so the brain can bind modules by identity.
                    AdStructure::CompleteLocalName(&format!("modit-{}-2", esp_hal::chip!())),
                ])
                .unwrap()
            )
        );
        println!("{:?}", ble.cmd_set_le_advertise_enable(true));

        println!("started advertising");

        // Any BLE client can write here, including a zero-length write, so the
        // first byte is matched rather than indexed.
        let mut write_led = |offset: usize, data: &[u8]| {
            println!("led write: offset {offset}, data {data:?}");
            match data.first() {
                Some(0) => led.set_low(),
                Some(_) => led.set_high(),
                None => println!("empty write to led_charac, ignoring"),
            }
        };

        // Reading the button characteristic is not part of the protocol -- the
        // brain only subscribes for notifications. Report zero bytes rather
        // than inventing a payload, and never assume the client's buffer size.
        //
        // TODO(plan-05): this is the natural place to expose the module's
        // identity (role, id, firmware version) once that exists.
        let mut read_button = |_offset: usize, _data: &mut [u8]| 0;
        gatt!([service {
            // Hardcoded, can be same for all modules?
            uuid: "937312e0-2354-11eb-9f10-fbc30a62cf30",
            characteristics: [
                characteristic {
                    name: "led_charac",
                    // Hardcoded, similar uuid for all leds
                    uuid: "927312e0-2354-11eb-9f10-fbc30a62cf30",
                    write: write_led,
                },
                characteristic {
                    name: "button_charac",
                    // Hardcoded, similar uuid for all buttons
                    uuid: "917312e0-2354-11eb-9f10-fbc30a62cf30",
                    notify: true,
                    read: read_button,
                },
            ],
        },]);

        let mut rng = bleps::no_rng::NoRng;
        let mut srv = AttributeServer::new(&mut ble, &mut gatt_attributes, &mut rng);
        let mut button_last_pressed_ms = 0;
        let mut button_was_high = false;
        loop {
            let mut notification = None;

            // Debounce in milliseconds rather than in loop iterations: one
            // iteration is one pass of the BLE work loop, so a tick-based
            // window stretches and shrinks with radio load.
            let button_is_high = button.is_high();
            let rising_edge = button_is_high && !button_was_high;
            button_was_high = button_is_high;

            if rising_edge && now().saturating_sub(button_last_pressed_ms) > DEBOUNCE_MS {
                button_last_pressed_ms = now();

                let mut cccd = [0u8; 1];
                let subscribed = srv
                    .get_characteristic_value(button_charac_notify_enable_handle, 0, &mut cccd)
                    .is_some()
                    && cccd[0] == 1;

                if subscribed {
                    println!("button pressed, notifying");
                    notification = Some(NotificationData::new(
                        button_charac_handle,
                        &b"Notification"[..],
                    ));
                } else {
                    println!("button pressed, but no client is subscribed");
                }
            }

            match srv.do_work_with_notification(notification) {
                Ok(res) => {
                    if let WorkResult::GotDisconnected = res {
                        break;
                    }
                }
                Err(err) => {
                    println!("{:?}", err);
                }
            }
        }
    }
}
