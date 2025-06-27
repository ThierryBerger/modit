mod ble;
//mod logic;
mod mock;

use crate::ble::notifier::BleNotifierClient;
use crate::mock::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use ble::*;
use btleplug::platform::Manager;
use shared::{Notifier, Writable};
use uuid::Uuid;

#[async_trait]
pub trait NotifierClient {
    async fn read(&mut self) -> Option<Vec<u8>>;
}

#[async_trait]
pub trait WritableClient {
    async fn write(&mut self, msg: &str);
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    let button = Notifier {
        service: "917312e0-2354-11eb-9f10-fbc30a62cf38",
        charac_notify_id: "0000ef01-0000-1000-8000-00805f9b34fb",
    };
    let modules = vec![Module::Notifier(button.clone())];
    let details = init_bluetooth(&modules).await.unwrap();

    let button_details = &details[&modules[0]];
    let coin = BleNotifierClient::new(button_details.0.clone()).await;
    //    let motor = BleMotorController::new(&adapter, "MotorModule", motor_uuid).await;

    //    let mut brain = Brain::new(coin, motor);
    //    brain.run_loop().await;
}
