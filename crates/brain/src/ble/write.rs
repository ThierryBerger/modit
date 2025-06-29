use btleplug::api::{Characteristic, Peripheral as _, WriteType};
use btleplug::platform::Peripheral;

pub async fn write(
    peripheral: &Peripheral,
    characteristic: Characteristic,
    message: &str,
) -> Result<(), btleplug::Error> {
    peripheral
        .write(
            &characteristic,
            message.as_bytes(),
            WriteType::WithoutResponse,
        )
        .await
}
