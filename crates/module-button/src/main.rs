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
//! # Why the button is an interrupt
//!
//! It used to be sampled once per pass of the BLE work loop. That works for a
//! finger -- a press lasts 50-200 ms, far longer than any plausible loop
//! overrun -- but the loop's period is *unbounded*, because
//! `do_work_with_notification` performs HCI I/O whose duration depends on what
//! the radio is doing. Sampling rate was therefore whatever was left over after
//! BLE.
//!
//! That stops being acceptable the moment an input is fast, and it is the same
//! argument `module-coin` already makes for its pulse line. An impact sensor on
//! a target struck by a ball is in contact for **4-6 ms**, which the old loop
//! would drop silently -- and a silently dropped hit reads to a player as having
//! missed.
//!
//! So the pin is an edge interrupt, debounced in the handler, and the main loop
//! only ever drains a count the handler already committed to. The rules live in
//! [`shared::input`], where they are tested on the host.
//!
//! Both edges are armed, not just the press: the release is what restarts the
//! debounce window, and without it the contact bounce as the button opens was
//! counted as a second press.
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

use core::cell::RefCell;

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
    delay::Delay,
    gpio::{Event as GpioEvent, Input, InputConfig, Io, Level, Output, OutputConfig, Pull},
    handler, main,
    rng::Rng,
    time,
    timer::timg::TimerGroup,
};
use esp_println::println;
use esp_wifi::{ble::controller::BleConnector, init};
use shared::input::EdgeLatch;

esp_bootloader_esp_idf::esp_app_desc!();

/// How long to ignore further edges after a button press, in milliseconds.
///
/// Applied in the interrupt handler, so it is a property of the *input* rather
/// than of how often the main loop happens to look. 50 ms suits a finger; a
/// plate that rings after being struck would want considerably more, and a
/// measurement rather than a guess.
const DEBOUNCE_MS: u32 = 50;

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

/// The button pin, so the interrupt handler can clear its own interrupt.
static BUTTON_PIN: Mutex<RefCell<Option<Input<'static>>>> = Mutex::new(RefCell::new(None));

/// Presses latched by the interrupt handler, waiting for the main loop.
static PRESSES: EdgeLatch = EdgeLatch::new();

/// Milliseconds since boot, truncated to 32 bits.
///
/// Truncated because the Xtensa core has no 64-bit atomics, so a timestamp
/// shared with an interrupt handler cannot be wider. It wraps after ~49 days;
/// [`EdgeLatch`] compares with `wrapping_sub`, so a wrap costs at most one
/// mis-timed debounce window rather than a hang.
fn now_ms() -> u32 {
    time::Instant::now().duration_since_epoch().as_millis() as u32
}

/// Latch one button press, or note that it was let go.
///
/// Deliberately tiny: read the clock and the pin, debounce, bump a counter,
/// clear the interrupt. Everything that could block -- BLE, logging -- happens
/// in the main loop, because this runs while the radio may be mid-transaction.
///
/// # Why both edges
///
/// The pin is armed for *any* edge, and which one fired is not reported, so the
/// level is read to tell them apart. Watching only the press was wrong: a
/// switch bounces when it opens exactly as it does when it closes, and by the
/// time a finger lets go -- 50-200 ms later -- the debounce window measured from
/// the press has long expired. The first bounce on release then counted as a
/// second press, and the game spent it on whatever lit next. See
/// [`EdgeLatch::record_release`].
///
/// Reading the level microseconds after the edge is not perfect: a contact that
/// re-opens inside the interrupt latency could be read as a release, costing one
/// press. That is far rarer than the bounce this fixes, and it fails towards a
/// missed press rather than an invented one -- the direction a game can survive.
#[handler]
fn button_edge() {
    let now = now_ms();

    critical_section::with(|cs| {
        if let Some(pin) = BUTTON_PIN.borrow_ref_mut(cs).as_mut() {
            if pin.is_high() {
                PRESSES.record(now, DEBOUNCE_MS);
            } else {
                PRESSES.record_release(now, DEBOUNCE_MS);
            }
            pin.clear_interrupt();
        }
    });
}

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
    let mut button = Input::new(
        // external button
        peripherals.GPIO33,
        config,
    );

    let mut bluetooth = peripherals.BT;
    let mut led = Output::new(peripherals.GPIO26, Level::Low, OutputConfig::default());

    self_test(&mut led, &button);

    // Route GPIO interrupts to `button_edge`, then arm *both* edges. The pin has
    // an internal pull-down and the button goes to 3V3, so a press pulls it up
    // and a release lets it fall -- see `docs/HARDWARE.md`. Only the rising edge
    // is a press; the falling one is watched because the release is what closes
    // the debounce window, and without it the bounce on release read as a second
    // press.
    let mut io = Io::new(peripherals.IO_MUX);
    io.set_interrupt_handler(button_edge);
    critical_section::with(|cs| {
        button.listen(GpioEvent::AnyEdge);
        BUTTON_PIN.borrow_ref_mut(cs).replace(button);
    });

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
        loop {
            let mut notification = None;

            // Drain what the interrupt handler latched. Debouncing already
            // happened there, so this is only ever "has a press been committed".
            if PRESSES.pending() > 0 {
                let mut cccd = [0u8; 1];
                let subscribed = srv
                    .get_characteristic_value(button_charac_notify_enable_handle, 0, &mut cccd)
                    .is_some()
                    && cccd[0] == 1;

                if subscribed {
                    // One press per pass, because `do_work_with_notification`
                    // carries one notification. Taking them all here would
                    // swallow the extras -- exactly the dropped input this
                    // firmware was changed to stop doing.
                    PRESSES.take_one();
                    println!("button pressed, notifying");
                    notification = Some(NotificationData::new(
                        button_charac_handle,
                        &b"Notification"[..],
                    ));
                } else {
                    // Nobody is listening, so these presses have nowhere to go.
                    // Discarding matches the previous behaviour and, more
                    // importantly, stops a button pressed while disconnected
                    // from firing a burst of stale notifications on connect.
                    let dropped = PRESSES.discard();
                    println!("{dropped} press(es) with no client subscribed, discarding");
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
