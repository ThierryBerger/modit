use async_trait::async_trait;

use crate::logic::*;
use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};

pub struct MockNotifierClient {
    pub mocked: Pin<Box<dyn Fn() -> Option<u32> + Send>>,
}

#[async_trait]
impl NotifierClient for MockNotifierClient {
    async fn read(&mut self) -> Option<u32> {
        (self.mocked)()
    }
}

pub struct MockWritableClient {
    pub mocked: Box<dyn Fn() + Send>,
}

#[async_trait]
impl WritableClient for MockWritableClient {
    async fn spin(&mut self, duration: Duration) {
        println!("Mock spin: {} seconds", duration.as_secs_f32());
        (self.mocked)()
    }
}
