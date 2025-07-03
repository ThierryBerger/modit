//! BLE Example
//!
//! - starts Bluetooth advertising
//! - offers one service with three characteristics (one is read/write, one is write only, one is read/write/notify)
//! - pressing the boot-button on a dev-board will send a notification if it is subscribed

//% FEATURES: esp-wifi esp-wifi/ble esp-hal/unstable
//% CHIPS: esp32 esp32s3 esp32c2 esp32c3 esp32c6 esp32h2

#![no_std]
#![no_main]

#[macro_use]
extern crate alloc;

use bleps::{
    ad_structure::{
        create_advertising_data, AdStructure, BR_EDR_NOT_SUPPORTED, LE_GENERAL_DISCOVERABLE,
    },
    att::Uuid,
    attribute_server::{AttributeServer, NotificationData, WorkResult},
    gatt, Ble, HciConnector,
};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    gpio::{Input, InputConfig, Io, Level, Output, OutputConfig, Pull},
    ledc::Ledc,
    main,
    rng::Rng,
    time,
    timer::timg::TimerGroup,
};
use esp_println::println;
use esp_wifi::{ble::controller::BleConnector, init};
use shared::{Notifier, Writable};

esp_bootloader_esp_idf::esp_app_desc!();

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
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
    let button = Input::new(peripherals.GPIO0, config);

    let mut bluetooth = peripherals.BT;
    let mut led = Output::new(peripherals.GPIO2, Level::Low, OutputConfig::default());

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
                    AdStructure::ServiceUuids16(&[Uuid::Uuid16(0x1809)]),
                    AdStructure::CompleteLocalName(&format!("modit-{}", esp_hal::chip!())),
                ])
                .unwrap()
            )
        );
        println!("{:?}", ble.cmd_set_le_advertise_enable(true));

        println!("started advertising");

        let mut wf2 = |offset: usize, data: &[u8]| {
            println!("RECEIVED: {} {:?}", offset, data);
            if data[0] == 0 {
                led.set_low();
            } else {
                led.set_high();
            }
        };

        let mut rf3 = |_offset: usize, data: &mut [u8]| {
            let to_send = format!("COIN:{}", 20);
            let bytes = to_send.as_bytes();
            let len = bytes.len();
            data[..len].copy_from_slice(bytes);
            len
        };
        gatt!([service {
            uuid: "937312e0-2354-11eb-9f10-fbc30a62cf38",
            characteristics: [
                characteristic {
                    name: "led_charac",
                    // Hardcoded, needs to be similar to your brain's writable.
                    uuid: "927312e0-2354-11eb-9f10-fbc30a62cf38",
                    write: wf2,
                },
                characteristic {
                    name: "button_charac",
                    // Hardcoded, needs to be similar to your brain's notifier.
                    uuid: "917312e0-2354-11eb-9f10-fbc30a62cf38",
                    notify: true,
                    read: rf3,
                },
            ],
        },]);

        let mut rng = bleps::no_rng::NoRng;
        let mut srv = AttributeServer::new(&mut ble, &mut gatt_attributes, &mut rng);
        let mut tick: u128 = 0;
        let mut button_last_pushed_tick = 0;
        loop {
            tick += 1;
            let mut notification = None;

            if button.is_low() {
                if tick.saturating_sub(button_last_pushed_tick) > 3 {
                    println!("sending notif?");
                    let mut cccd = [0u8; 1];
                    if let Some(1) = srv.get_characteristic_value(
                        button_charac_notify_enable_handle,
                        0,
                        &mut cccd,
                    ) {
                        // if notifications enabled
                        if cccd[0] == 1 {
                            println!("sending notif!");
                            notification = Some(NotificationData::new(
                                button_charac_handle,
                                &b"Notification"[..],
                            ));
                        }
                    }
                }
                button_last_pushed_tick = tick;
            };

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
