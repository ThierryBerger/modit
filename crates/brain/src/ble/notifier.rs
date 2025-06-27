use async_trait::async_trait;
use btleplug::api::{Central, Characteristic, Manager as _, Peripheral as _, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral};
use uuid::Uuid;

use crate::NotifierClient;

pub struct BleNotifierClient {
    peripheral: Peripheral,
}

impl BleNotifierClient {
    pub async fn new(peripheral: Peripheral) -> Self {
        Self { peripheral }
    }
}

#[async_trait]
impl NotifierClient for BleNotifierClient {
    async fn read(&mut self) -> Option<Vec<u8>> {
        use futures::StreamExt;
        // Print the first 4 notifications received.
        let mut notification_stream = self.peripheral.notifications().await.ok()?;
        // Process while the BLE connection is not broken or stopped.
        if let Some(data) = notification_stream.next().await {
            // Optional: check the characteristic which triggered the data.
            // as we only subscribe to 1 characteristic it's fine to skip this check.
            println!(
                "Received data from {:?} [{:?}]: {:?}",
                self.peripheral.address(),
                data.uuid,
                data.value
            );
            return Some(data.value);
        }
        None
    }
}
