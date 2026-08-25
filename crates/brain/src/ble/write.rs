use btleplug::api::{Characteristic, Peripheral as _, WriteType};
use btleplug::platform::Peripheral;
use log::trace;

pub async fn write(
    peripheral: &Peripheral,
    characteristic: &Characteristic,
    message: &[u8],
) -> Result<(), btleplug::Error> {
    trace!(
        "writing {message:?} to {} on {}",
        characteristic.uuid,
        peripheral.address()
    );
    peripheral
        .write(characteristic, message, WriteType::WithoutResponse)
        .await
}
