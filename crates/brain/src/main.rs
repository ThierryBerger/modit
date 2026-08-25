mod ble;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use ble::*;
use btleplug::api::{Characteristic, Manager as _, Peripheral as _};
use btleplug::platform::{Manager, Peripheral};
use futures::join;
use shared::{Notifier, Writable};
use tokio::{sync::Mutex, task};

use rand::RngCore;
use rand::SeedableRng;
use rand::rngs::SmallRng;

#[derive(Clone, Debug)]
pub struct ButtonLed {
    pub button: Notifier,
    pub led: Writable,
}

#[derive(Clone, Debug)]
pub struct ButtonDetails {
    pub button: Characteristic,
    pub led: Characteristic,
}

impl ModuleDefinition<ButtonDetails> for ButtonLed {
    async fn with_peripheral(
        &self,
        peripheral: &btleplug::platform::Peripheral,
    ) -> Option<ButtonDetails> {
        let button = (self.button.with_peripheral(peripheral).await)?;
        let led = (self.led.with_peripheral(peripheral).await)?;
        Some(ButtonDetails { button, led })
    }
}

async fn is_connected(peripheral: &Peripheral) -> bool {
    // edge case: https://github.com/deviceplug/btleplug/issues/277
    tokio::select! {
        _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
            eprintln!("peripheral timed out");
            false
        }
        // connect to the device
        _ = peripheral.is_connected() => {
            true
        }
    }
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    let button_led = ButtonLed {
        button: Notifier {
            service: "937312e0-2354-11eb-9f10-fbc30a62cf30",
            charac_notify_id: "917312e0-2354-11eb-9f10-fbc30a62cf30",
        },
        led: Writable {
            service: "937312e0-2354-11eb-9f10-fbc30a62cf30",
            charac_write_id: "927312e0-2354-11eb-9f10-fbc30a62cf30",
        },
    };
    let modules = vec![button_led.clone(), button_led];

    let manager = Manager::new().await.unwrap();
    let adapter_list = manager.adapters().await.unwrap();

    loop {
        let details = init_bluetooth(&adapter_list, &modules).await.unwrap();

        if details.iter().any(|p| p.is_none()) {
            // Retry.
            continue;
        }
        let buttons = details.into_iter().flatten().collect::<Vec<_>>();

        // reset all buttons
        for button in &buttons {
            write::write(&button.peripheral, &button.module.led, &[0])
                .await
                .unwrap();
        }
        // light up random button
        let mut rng = SmallRng::seed_from_u64(42);
        let random_sleep_amount = rng.next_u64() % 1500 + 1000;
        tokio::time::sleep(std::time::Duration::from_millis(random_sleep_amount)).await;
        let rand_button_index = rng.next_u64() % buttons.len() as u64;
        let details = &buttons[rand_button_index as usize];
        write::write(&details.peripheral, &details.module.led, &[1])
            .await
            .unwrap();

        let expected_button = Arc::new(Mutex::new(Some(rand_button_index)));
        let rng = Arc::new(Mutex::new(rng));

        let mut tasks = Vec::new();
        let should_abort = Arc::new(AtomicBool::new(false));
        for (i, button) in buttons.iter().cloned().enumerate() {
            let rng = rng.clone();
            let buttons = buttons.clone();
            let expected_button = expected_button.clone();
            let should_abort = should_abort.clone();
            let task = task::spawn(async move {
                loop {
                    if !is_connected(&button.peripheral).await {
                        should_abort.store(true, std::sync::atomic::Ordering::Relaxed);
                        println!("Button {i} was disconnected!");
                    }
                    if should_abort.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                    if let Some(value) = read::read_notification(&button.peripheral).await {
                        let Some(expected_button_index) = *expected_button.lock().await else {
                            println!("button received ; but not expecting it yet!");
                            continue;
                        };
                        if i != expected_button_index as usize {
                            println!("Incorrect button pressed.");
                            continue;
                        }

                        println!("value received: {:?}", value);
                        write::write(&button.peripheral, &button.module.led, &[0])
                            .await
                            .unwrap();

                        *expected_button.lock().await = None;
                        let rng = rng.clone();
                        let buttons = buttons.clone();
                        let expected_button_captured = expected_button.clone();
                        task::spawn(async move {
                            let mut rng = rng.lock().await;
                            let random_sleep_amount = rng.next_u64() % 500 + 500;
                            tokio::time::sleep(std::time::Duration::from_millis(
                                random_sleep_amount,
                            ))
                            .await;
                            let rand_button_index = rng.next_u64() % buttons.len() as u64;
                            let details = &buttons[rand_button_index as usize];
                            write::write(&details.peripheral, &details.module.led, &[1])
                                .await
                                .unwrap();
                            *expected_button_captured.lock().await = Some(rand_button_index);
                        });
                    } else {
                        //  println!("nothing received from {i}");
                    }
                    //tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            });
            tasks.push(task);
        }
        for t in tasks {
            let _ = join!(t);
        }
    }
}
