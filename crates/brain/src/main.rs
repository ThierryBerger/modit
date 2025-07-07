mod ble;

use std::{sync::Arc, time::Duration};

use ble::*;
use shared::{Notifier, Writable};
use tokio::{sync::Mutex, task};

use rand::RngCore;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

#[derive(Clone, Debug)]
pub struct ButtonLed {
    pub button: Notifier,
    pub led: Writable,
}

#[derive(Clone, Debug)]
pub struct ButtonDetails {
    pub button: ModuleDetails,
    pub led: ModuleDetails,
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    let button1 = ButtonLed {
        button: Notifier {
            service: "937312e0-2354-11eb-9f10-fbc30a62cf38",
            charac_notify_id: "917312e0-2354-11eb-9f10-fbc30a62cf38",
        },
        led: Writable {
            service: "937312e0-2354-11eb-9f10-fbc30a62cf38",
            charac_write_id: "927312e0-2354-11eb-9f10-fbc30a62cf38",
        },
    };
    let button2 = ButtonLed {
        button: Notifier {
            service: "937312e0-2354-11eb-9f10-fbc30a62cf30",
            charac_notify_id: "917312e0-2354-11eb-9f10-fbc30a62cf30",
        },
        led: Writable {
            service: "937312e0-2354-11eb-9f10-fbc30a62cf30",
            charac_write_id: "927312e0-2354-11eb-9f10-fbc30a62cf30",
        },
    };
    let modules = vec![
        Module::Notifier(button1.button.clone()),
        Module::Writable(button1.led.clone()),
        Module::Notifier(button2.button.clone()),
        Module::Writable(button2.led.clone()),
    ];
    let details = init_bluetooth(&modules).await.unwrap();

    let buttons = [
        ButtonDetails {
            button: details[&modules[0]].clone(),
            led: details[&modules[1]].clone(),
        },
        ButtonDetails {
            button: details[&modules[2]].clone(),
            led: details[&modules[3]].clone(),
        },
    ];

    for button in &buttons {
        write::write(&button.led.peripheral, &button.led.characteristic, &[1])
            .await
            .unwrap();
    }

    let expected_button = Arc::new(Mutex::new(Some(0)));
    let rng = Arc::new(Mutex::new(SmallRng::seed_from_u64(42)));
    loop {
        for (i, button) in buttons.iter().enumerate() {
            // FIXME: read is blocking for next notif, call a non blocking variant!
            if let Some(value) = read::read_notification(&button.button.peripheral).await {
                let Some(expected_button_index) = *expected_button.lock().await else {
                    println!("button received ; but not expecting it yet!");
                    continue;
                };
                if i != expected_button_index {
                    println!("Incorrect button pressed.");
                    continue;
                }

                println!("value received: {:?}", value);
                write::write(&button.led.peripheral, &button.led.characteristic, &[0])
                    .await
                    .unwrap();
                let expected_button_captured = expected_button.clone();

                *expected_button.lock().await = None;
                let rng = rng.clone();
                let buttons = buttons.clone();
                task::spawn(async move {
                    let mut rng = rng.lock().await;
                    let random_sleep_amount = rng.next_u64() % 1500 + 1000;
                    tokio::time::sleep(std::time::Duration::from_millis(random_sleep_amount)).await;
                    let rand_button_index = rng.next_u64() % buttons.len() as u64;
                    let details = &buttons[rand_button_index as usize];
                    write::write(&details.led.peripheral, &details.led.characteristic, &[1])
                        .await
                        .unwrap();
                    *expected_button_captured.lock().await = Some(rand_button_index as usize);
                });
            } else {
                println!("nothing received from {i}");
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
