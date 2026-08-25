use std::time::Duration;

use btleplug::api::{Peripheral as _, ValueNotification};
use btleplug::platform::Peripheral;
use log::{debug, trace, warn};
use tokio::time::timeout;

/// How long to wait for a notification before giving the caller a turn.
///
/// TODO(plan-04): this whole function is the wrong shape. It opens a fresh
/// notification stream on every call and drops it, so presses that land between
/// polls are lost. The stream should be opened once at bind time and held.
const POLL_WINDOW: Duration = Duration::from_millis(200);

pub async fn read_notification(peripheral: &Peripheral) -> Option<ValueNotification> {
    use futures::StreamExt;

    let mut notification_stream = match peripheral.notifications().await {
        Ok(stream) => stream,
        Err(err) => {
            warn!("could not open the notification stream: {err}");
            return None;
        }
    };

    match timeout(POLL_WINDOW, notification_stream.next()).await {
        Ok(Some(data)) => {
            // We only ever subscribe to one characteristic per peripheral, so
            // there is nothing to disambiguate yet.
            // TODO(plan-04): filter by characteristic UUID once that stops
            // being true.
            debug!(
                "notification from {} [{}]: {:?}",
                peripheral.address(),
                data.uuid,
                data.value
            );
            Some(data)
        }
        // The stream ended: the peripheral is gone or unsubscribed.
        Ok(None) => {
            debug!("notification stream from {} ended", peripheral.address());
            None
        }
        // Nothing arrived in this window, which is the common case.
        Err(_elapsed) => {
            trace!("no notification from {} this round", peripheral.address());
            None
        }
    }
}
