mod ble;

use std::{sync::Arc, time::Duration};

use ble::*;
use shared::{Notifier, Writable};
use tokio::{sync::Mutex, task};

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    let button = Notifier {
        service: "937312e0-2354-11eb-9f10-fbc30a62cf38",
        charac_notify_id: "917312e0-2354-11eb-9f10-fbc30a62cf38",
    };

    let led = Writable {
        service: "937312e0-2354-11eb-9f10-fbc30a62cf38",
        charac_write_id: "927312e0-2354-11eb-9f10-fbc30a62cf38",
    };
    let modules = vec![
        Module::Notifier(button.clone()),
        Module::Writable(led.clone()),
    ];
    let details = init_bluetooth(&modules).await.unwrap();

    let button_details = &details[&modules[0]];

    let led_details = &details[&modules[1]];
    let is_button_active_mutex = Arc::new(Mutex::new(true));
    write::write(&led_details.peripheral, &led_details.characteristic, &[1])
        .await
        .unwrap();
    loop {
        if let Some(value) = read::read_notification(&button_details.peripheral).await {
            let is_button_active = *is_button_active_mutex.lock().await;
            if !is_button_active {
                println!("button received ; but not expecting it yet!");
            } else {
                println!("value received: {:?}", value);
                write::write(&led_details.peripheral, &led_details.characteristic, &[0])
                    .await
                    .unwrap();
                let led_details = led_details.clone();
                let is_button_active_mutex = is_button_active_mutex.clone();

                *is_button_active_mutex.lock().await = false;
                task::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                    write::write(&led_details.peripheral, &led_details.characteristic, &[1])
                        .await
                        .unwrap();
                    *is_button_active_mutex.lock().await = true;
                });
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
