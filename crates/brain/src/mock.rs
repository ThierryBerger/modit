use async_trait::async_trait;

use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{NotifierClient, WritableClient};

pub struct MockNotifierClient {
    pub mocked: Pin<Box<dyn Fn() -> Option<Vec<u8>> + Send>>,
}

#[async_trait]
impl NotifierClient for MockNotifierClient {
    async fn read(&mut self) -> Option<Vec<u8>> {
        (self.mocked)()
    }
}

pub struct MockWritableClient {
    pub mocked: Box<dyn Fn() + Send>,
}

#[async_trait]
impl WritableClient for MockWritableClient {
    async fn write(&mut self, msg: &str) {
        println!("Written: {}", msg);
        (self.mocked)()
    }
}
