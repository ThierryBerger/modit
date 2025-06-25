use std::time::Duration;

use crate::logic::{Coin, CoinReceiver, MotorController};
use async_trait::async_trait;
use btleplug::api::{Central, Characteristic, Manager as _, Peripheral as _, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral};
use uuid::Uuid;

pub struct BleWritableClient {
    motor: Peripheral,
    motor_char: Characteristic,
}

impl BleWritableClient {
    pub async fn new(adapter: &Adapter, motor_name: &str, uuid: Uuid) -> Self {
        let peripherals = adapter.peripherals().await.unwrap();
        todo!();
        /*
        let motor = peripherals
            .into_iter()
            .find(|p| {
                p.properties().now_or_never()?.unwrap().local_name == Some(motor_name.to_string())
            })
            .expect("Motor module not found");

        motor.connect().await.unwrap();
        motor.discover_services().await.unwrap();

        let motor_char = motor
            .characteristics()
            .iter()
            .find(|c| c.uuid == uuid)
            .expect("Motor characteristic not found")
            .clone();

        Self { motor, motor_char }*/
    }
}

#[async_trait]
impl WritableClient for BleWritableClient {
    async fn write(&mut self, message: &str) {
        self.motor
            .write(&self.motor_char, msg.as_bytes(), WriteType::WithoutResponse)
            .await
            .expect("Failed to send motor command");
    }
}
