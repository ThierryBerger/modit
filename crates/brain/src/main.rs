mod ble;

use std::time::Duration;

use ble::*;
use shared::Notifier;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    let button = Notifier {
        service: "937312e0-2354-11eb-9f10-fbc30a62cf38",
        charac_notify_id: "917312e0-2354-11eb-9f10-fbc30a62cf38",
    };
    let modules = vec![Module::Notifier(button.clone())];
    let details = init_bluetooth(&modules).await.unwrap();

    let button_details = &details[&modules[0]];

    loop {
        if let Some(coin) = read::read_notification(&button_details.peripheral).await {
            println!("value received: {:?}", coin);
            let duration = Duration::from_millis(match coin.value[0] {
                100 => 2000,
                50 => 1000,
                _ => 500,
            });
        }

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
