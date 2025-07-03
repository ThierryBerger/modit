use btleplug::api::{Characteristic, Peripheral as _, WriteType};
use btleplug::platform::Peripheral;

pub async fn write(
    peripheral: &Peripheral,
    characteristic: &Characteristic,
    message: &[u8],
) -> Result<(), btleplug::Error> {
    peripheral
        .write(characteristic, message, WriteType::WithoutResponse)
        .await
}
