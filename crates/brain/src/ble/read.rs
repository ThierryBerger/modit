use btleplug::api::{Peripheral as _, ValueNotification};
use btleplug::platform::Peripheral;

pub async fn read_notification(peripheral: &Peripheral) -> Option<ValueNotification> {
    use futures::StreamExt;
    // Print the first 4 notifications received.
    let mut notification_stream = peripheral.notifications().await.ok()?;
    // Process while the BLE connection is not broken or stopped.
    if let Some(data) = notification_stream.next().await {
        // Optional: check the characteristic which triggered the data.
        // as we only subscribe to 1 characteristic it's fine to skip this check.
        println!(
            "Received data from {:?} [{:?}]: {:?}",
            peripheral.address(),
            data.uuid,
            data.value
        );
        return Some(data);
    }
    None
}
