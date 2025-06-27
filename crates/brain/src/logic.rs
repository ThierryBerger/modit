use async_trait::async_trait;
use std::time::Duration;

pub struct Brain<R, M>
where
    R: CoinReceiver + Send,
    M: MotorController + Send,
{
    pub coin_input: R,
    pub motor: M,
}

impl<R, M> Brain<R, M>
where
    R: CoinReceiver + Send,
    M: MotorController + Send,
{
    pub fn new(coin_input: R, motor: M) -> Self {
        Self { coin_input, motor }
    }

    pub async fn run_loop(&mut self) {
        loop {
            if let Some(coin) = self.coin_input.read_coin().await {
                println!("Coin received: {:?}", coin);
                let duration = Duration::from_millis(match coin {
                    100 => 2000,
                    50 => 1000,
                    _ => 500,
                });
                self.motor
                    .spin(format!("SPIN:{}", duration.as_millis()).as_bytes())
                    .await;
            }

            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }
}
