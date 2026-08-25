//! Firmware for a modit button module: one button, one LED (ESP32).
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
    delay::Delay,
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

/// What kind of module this firmware is. Part of the advertised name.
const MODIT_ROLE: &str = "button";

/// Which physical board this is, baked in at flash time.
///
/// Every board runs identical firmware, so without this they all advertise the
/// same name and the brain cannot tell them apart -- "button 0" would be a
/// different box on every run.
const MODIT_ID: &str = match option_env!("MODIT_ID") {
    Some(id) => id,
    None => panic!(
        "MODIT_ID is not set, so this board would be indistinguishable from every \
         other one. Flash with `just flash <id>` from the repository root, for \
         example `just flash a`."
    ),
};

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

/// Blink the LED and report the button, so wiring can be checked the moment the
/// board is flashed.
///
/// Without this, a reversed LED or a button wired to GND instead of 3V3 is not
/// noticed until the whole system is running, several steps later, where it looks
/// like a BLE problem. Costs about a second at boot.
fn self_test(led: &mut Output<'_>, button: &Input<'_>) {
    let delay = Delay::new();

    println!("self-test: blinking the LED three times (GPIO26)");
    for _ in 0..3 {
        led.set_high();
        delay.delay_millis(150);
        led.set_low();
        delay.delay_millis(150);
    }

    // The pin has a pull-down, so it idles low. Reading high here means the
    // button is either held or -- much more likely -- wired to GND.
    if button.is_high() {
        println!(
            "self-test: WARNING -- button (GPIO33) reads HIGH at rest. If you are \
             not holding it, it is wired to GND; it should go to 3V3."
        );
    } else {
        println!("self-test: button (GPIO33) reads low at rest, as expected");
    }
    println!("self-test: press the button now -- you should see a line for each press");
}

#[main]
fn main() -> ! {
    esp_println::logger::init_logger_from_env();

    println!(
        "modit {MODIT_ROLE} module, id {MODIT_ID:?}, on {}",
        esp_hal::chip!()
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

    let config = InputConfig::default().with_pull(Pull::Down);
    let button = Input::new(
        // external button
        peripherals.GPIO33,
        config,
    );

    let mut bluetooth = peripherals.BT;
    let mut led = Output::new(peripherals.GPIO26, Level::Low, OutputConfig::default());

    self_test(&mut led, &button);

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
                    AdStructure::CompleteLocalName(&format!("modit-{MODIT_ROLE}-{MODIT_ID}")),
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
        // These must stay in step with `shared::uuids`. They cannot reference it
        // directly: `gatt!` parses UUIDs when it expands, and generates the handle
        // identifiers used below from them, so it only accepts string literals.
        //
        // A host-side test (`firmware_uuids_match_shared`, in brain) reads this
        // file and fails if any of them drifts.
        gatt!([service {
            // shared::uuids::SERVICE
            uuid: "937312e0-2354-11eb-9f10-fbc30a62cf30",
            characteristics: [
                characteristic {
                    name: "led_charac",
                    // shared::uuids::LED_WRITE
                    uuid: "927312e0-2354-11eb-9f10-fbc30a62cf30",
                    write: write_led,
                },
                characteristic {
                    name: "button_charac",
                    // shared::uuids::BUTTON_NOTIFY
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
