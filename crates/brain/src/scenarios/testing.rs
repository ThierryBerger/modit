//! Test support shared by every scenario's tests: a virtual-time wait and a
//! simulated bench of buttons.

use std::time::Duration;

use crate::link::ModuleId;
use crate::link::sim::{self, SimHandle, SimLink};
use crate::runtime;

/// Wait for a condition to become true, or give up.
///
/// Every test here runs with `start_paused = true`, so `tokio::time` is virtual:
/// when every task is idle, the clock jumps to the next deadline. That means the
/// scenario's real sleeps cost nothing, and -- more importantly -- these tests do
/// not depend on wall-clock timing, so they cannot go flaky when the machine is
/// busy. Deadlines below are in virtual time.
pub async fn eventually<F: FnMut() -> bool>(mut condition: F, what: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        if condition() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("timed out waiting for {what}");
}

pub fn start_with<S, F>(
    ids: &[&str],
    scenario: S,
) -> (SimHandle, tokio::task::JoinHandle<anyhow::Result<()>>)
where
    S: FnMut(std::sync::Arc<runtime::Modules>) -> F + Send + 'static,
    F: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
{
    let specs = ids
        .iter()
        .map(|id| (ModuleId(id.to_string()), sim::button(1, 1)))
        .collect();
    let (link, handle) = SimLink::new(specs);
    let task = tokio::spawn(runtime::run(link.quiet(), scenario));
    (handle, task)
}
