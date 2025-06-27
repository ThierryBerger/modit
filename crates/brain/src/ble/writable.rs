use std::time::Duration;

use async_trait::async_trait;
use btleplug::api::{Central, Characteristic, Manager as _, Peripheral as _, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral};
use uuid::Uuid;

use crate::WritableClient;

pub struct BleWritableClient {
    peripheral: Peripheral,
    characteristic: Characteristic,
}

impl BleWritableClient {
    pub async fn new(peripheral: Peripheral, characteristic: Characteristic) -> Self {
        Self {
            peripheral,
            characteristic,
        }
    }
}

#[async_trait]
impl WritableClient for BleWritableClient {
    async fn write(&mut self, message: &str) {
        self.peripheral
            .write(
                &self.characteristic,
                message.as_bytes(),
                WriteType::WithoutResponse,
            )
            .await
            .expect("Failed to send motor command");
    }
}
