use btleplug::api::{Peripheral as _, ValueNotification};
use btleplug::platform::Peripheral;
use futures::Stream;
use log::debug;

/// The notification stream for one peripheral.
///
/// Opened **once**, when the module is bound, and held for as long as the module
/// is in play. It used to be reopened on every poll and dropped 200 ms later,
/// which silently lost any notification that arrived in the gap -- a button
/// press that fell between two polls simply never happened.
pub type Notifications = std::pin::Pin<Box<dyn Stream<Item = ValueNotification> + Send>>;

/// Open the notification stream for a peripheral.
pub async fn notifications(peripheral: &Peripheral) -> Result<Notifications, btleplug::Error> {
    debug!("opening notification stream for {}", peripheral.address());
    peripheral.notifications().await
}
