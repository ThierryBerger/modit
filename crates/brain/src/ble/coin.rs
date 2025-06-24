use crate::logic::{Coin, CoinReceiver, MotorController};
use async_trait::async_trait;
use btleplug::api::{Central, Characteristic, Manager as _, Peripheral as _, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral};
use uuid::Uuid;

pub struct BleCoinReceiver {
    coin: Peripheral,
    charac: Characteristic,
}

impl BleCoinReceiver {
    pub async fn new(adapter: &Adapter, coin_name: &str, uuid: Uuid) -> anyhow::Result<()> {
        coin.connect().await.unwrap();
        coin.discover_services().await.unwrap();

        let charac = coin
            .characteristics()
            .into_iter()
            .find(|c| c.uuid == uuid)
            .expect("Coin notify characteristic not found");

        coin.subscribe(&charac).await.unwrap();
        let notif_stream = coin.notifications().await.unwrap();

        Self {
            coin,
            charac,
            notif_stream,
        }
    }
}

#[async_trait]
impl CoinReceiver for BleCoinReceiver {
    async fn read_coin(&mut self) -> Option<u32> {
        use futures::StreamExt;
        if let Some(notification) = self.notif_stream.next().await {
            let msg = String::from_utf8_lossy(&notification.value);
            if msg.starts_with("COIN:") {
                if let Some(value) = msg[5..].parse::<u32>().ok() {
                    return Some(value);
                }
            }
        }
        None
    }
}
