use crate::logic::{Coin, CoinReceiver, MotorController, NotifierClient};
use async_trait::async_trait;
use btleplug::api::{Central, Characteristic, Manager as _, Peripheral as _, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral};
use uuid::Uuid;

pub struct BleNotifierClient {
    notifier: Peripheral,
}

impl BleNotifierClient {
    pub async fn new(peripheral: &Peripheral) -> anyhow::Result<()> {
        Self { notifier }
    }
}

#[async_trait]
impl NotifierClient for BleNotifierClient {
    async fn read(&mut self) -> Option<u32> {
        use futures::StreamExt;
        // Print the first 4 notifications received.
        let mut notification_stream = self.peripheral.notifications().await?;
        // Process while the BLE connection is not broken or stopped.
        while let Some(data) = notification_stream.next().await {
            // Optional: check the characteristic which triggered the data.
            // as we only subscribe to 1 characteristic it's fine to skip this check.
            println!(
                "Received data from {:?} [{:?}]: {:?}",
                local_name, data.uuid, data.value
            );
            if let Some(value) = msg[0..].parse::<u32>().ok() {
                return Some(value);
            }
        }
        None
    }
}
