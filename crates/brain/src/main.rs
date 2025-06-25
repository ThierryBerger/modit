mod ble;
mod logic;
mod mock;

use crate::ble::notifier::BleCoinReceiver;
use crate::logic::*;
use crate::mock::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ble::*;
use btleplug::platform::Manager;
use shared::{Notifier, Writable};
use uuid::Uuid;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    init_bluetooth().await;
    let button: Notifier = ron::de::from_str(include_str!("../../../assets/simple_button.ron"));
    let writable: Writable = ron::de::from_str(include_str!("../../../assets/simple_writable.ron"));

    let coin_uuid = Uuid::parse_str(button.service).unwrap();
    let writable_uuid = Uuid::parse_str("0000ef01-0000-1000-8000-00805f9b34fb").unwrap();

    let coin = BleCoinReceiver::new(&adapter, "CoinModule", coin_uuid).await;
    //    let motor = BleMotorController::new(&adapter, "MotorModule", motor_uuid).await;

    //    let mut brain = Brain::new(coin, motor);
    //    brain.run_loop().await;
}
