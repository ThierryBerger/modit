use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::Result;
use dotenvy_macro::dotenv;
use esp_idf_hal::gpio::*;
use esp_idf_hal::interrupt;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::ble::*;
use esp_idf_sys as _;
use log::*;

// Track how many pulses happened
static PULSE_COUNT: AtomicU8 = AtomicU8::new(0);

// Track last pulse timestamp (ms since boot)
static LAST_PULSE_MS: AtomicU64 = AtomicU64::new(0);

fn setup_interrupt(pin: Gpio4) -> Result<PinDriver<'static, Gpio4, Input>> {
    let mut driver = PinDriver::input(pin)?;
    driver.set_pull(Pull::Up)?;
    driver.set_interrupt_type(InterruptType::NegEdge)?;

    interrupt::subscribe(
        move || {
            let now = esp_idf_hal::sys::esp_timer_get_time() / 1000; // ms since boot

            let last = LAST_PULSE_MS.load(Ordering::Relaxed);
            if now - last > 100 {
                PULSE_COUNT.fetch_add(1, Ordering::SeqCst);
                LAST_PULSE_MS.store(now, Ordering::Relaxed);
            }
        },
        driver.pin(),
    )?;

    Ok(driver)
}

fn main() {
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let _coin_pin = setup_interrupt(peripherals.pins.gpio4)?; // keep alive

    let ble = BlePeripheral::new("CoinModule")?;
    let mut service = ble.add_service(uuid128!(dotenv!("COIN_SERVICE_ID")))?;
    let mut charac =
        service.add_notify_characteristic(uuid128!(dotenv!("COIN_CHARACTERISTIC_ID")))?;
    ble.start_advertising()?;
    info!("CoinModule with interrupts is advertising");

    loop {
        let count = PULSE_COUNT.load(Ordering::SeqCst);
        let last_pulse_ms = LAST_PULSE_MS.load(Ordering::Relaxed);
        let now = esp_idf_hal::sys::esp_timer_get_time() / 1000;

        if count > 0 && now - last_pulse_ms > 1000 {
            // Timeout passed → interpret coin
            let value = match count {
                1 => 10,
                2 => 20,
                3 => 50,
                4 => 100,
                5 => 200,
                _ => {
                    warn!("Unknown pulse count: {}", count);
                    0
                }
            };

            if value > 0 {
                let msg = format!("COIN:{}", value);
                charac.notify(msg.as_bytes())?;
                info!("Sent BLE: {}", msg);
            }

            PULSE_COUNT.store(0, Ordering::SeqCst);
        }

        sleep(Duration::from_millis(50));
    }
}
