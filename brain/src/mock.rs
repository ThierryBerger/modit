use async_trait::async_trait;

use crate::logic::*;
use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};

pub struct MockCoinInput {
    pub mocked: Pin<Box<dyn Fn() -> Option<u32> + Send>>,
}

#[async_trait]
impl CoinReceiver for MockCoinInput {
    async fn read_coin(&mut self) -> Option<u32> {
        (self.mocked)()
    }
}

pub struct MockMotorOutput {
    pub mocked: Box<dyn Fn() + Send>,
}

#[async_trait]
impl MotorController for MockMotorOutput {
    async fn spin(&mut self, duration: Duration) {
        println!("Mock spin: {} seconds", duration.as_secs_f32());
        (self.mocked)()
    }
}
