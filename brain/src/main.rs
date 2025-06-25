mod ble;
mod logic;
mod mock;

use crate::logic::*;
use crate::mock::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ble::*;
use btleplug::platform::Manager;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    init_bluetooth().await;

    let coin_uuid = Uuid::parse_str("00005678-0000-1000-8000-00805f9b34fb").unwrap();
    let motor_uuid = Uuid::parse_str("0000ef01-0000-1000-8000-00805f9b34fb").unwrap();

    //    let coin = BleCoinReceiver::new(&adapter, "CoinModule", coin_uuid).await;
    //    let motor = BleMotorController::new(&adapter, "MotorModule", motor_uuid).await;

    //    let mut brain = Brain::new(coin, motor);
    //    brain.run_loop().await;
}
