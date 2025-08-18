pub mod read;
pub mod write;

use core::time::Duration;

use btleplug::api::{Central, CharPropFlags, Characteristic, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Peripheral};
use shared::{Notifier, Writable};
use uuid::Uuid;

pub struct IdentifiedModule<T> {
    pub peripheral: Peripheral,
    pub module: T,
}

impl<T> Clone for IdentifiedModule<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            peripheral: self.peripheral.clone(),
            module: self.module.clone(),
        }
    }
}

impl ModuleDefinition<Characteristic> for Notifier {
    async fn with_peripheral(&self, peripheral: &Peripheral) -> Option<Characteristic> {
        for characteristic in peripheral.characteristics() {
            println!("Checking notifier characteristic {characteristic:?}");
            // Subscribe to notifications from the characteristic with the selected
            // UUID.
            if characteristic.uuid == Uuid::parse_str(self.charac_notify_id).unwrap()
                && characteristic.properties.contains(CharPropFlags::NOTIFY)
            {
                println!("Subscribing to characteristic {:?}", characteristic.uuid);
                peripheral.subscribe(&characteristic).await.ok()?;
                return Some(characteristic);
            }
        }
        None
    }
}
impl ModuleDefinition<Characteristic> for Writable {
    async fn with_peripheral(&self, peripheral: &Peripheral) -> Option<Characteristic> {
        for characteristic in peripheral.characteristics() {
            println!("Checking writable characteristic {characteristic:?}");
            if characteristic.uuid == Uuid::parse_str(self.charac_write_id).unwrap()
                && characteristic.properties.contains(CharPropFlags::WRITE)
            {
                return Some(characteristic);
            }
        }
        None
    }
}

pub trait ModuleDefinition<T> {
    async fn with_peripheral(&self, peripheral: &Peripheral) -> Option<T>;
}

pub async fn init_bluetooth<T>(
    adapter_list: &[Adapter],
    modules: &[impl ModuleDefinition<T>],
) -> anyhow::Result<Vec<Option<IdentifiedModule<T>>>> {
    if adapter_list.is_empty() {
        eprintln!("No Bluetooth adapters found");
    }

    let mut final_modules: Vec<Option<IdentifiedModule<T>>> =
        (0..modules.len()).map(|_| None).collect::<Vec<_>>();
    for adapter in dbg!(adapter_list).iter() {
        println!("Starting scan...");
        adapter
            .start_scan(ScanFilter::default())
            .await
            .expect("Can't scan BLE adapter for connected devices...");
        tokio::time::sleep(Duration::from_secs(3)).await;
        let peripherals = adapter.peripherals().await?;
        if peripherals.is_empty() {
            eprintln!("->>> BLE peripheral devices were not found, sorry. Exiting...");
        } else {
            // All peripheral devices in range.
            for peripheral in peripherals.iter() {
                if final_modules.iter().any(|m| {
                    let Some(m) = m else {
                        return false;
                    };
                    m.peripheral.id() == peripheral.id()
                }) {
                    continue;
                }
                let properties = peripheral.properties().await?;
                let is_connected = peripheral.is_connected().await?;
                let local_name = properties
                    .unwrap()
                    .local_name
                    .unwrap_or(String::from("(peripheral name unknown)"));
                if !local_name.starts_with("modit") {
                    continue;
                }

                println!("Found matching peripheral {:?}...", &local_name);
                if !is_connected {
                    println!("Trying to connect...");
                    // Connect if we aren't already connected.
                    if let Err(err) = peripheral.connect().await {
                        eprintln!("Error connecting to peripheral, skipping: {err}");
                        continue;
                    }
                    println!("Connection with {local_name} has reported success.");
                }
                println!("Discover peripheral {local_name:?} services...");
                peripheral.discover_services().await?;
                for (i, module) in modules.iter().enumerate() {
                    if final_modules[i].is_some() {
                        continue;
                    }
                    // Check if it's the peripheral we want.
                    if let Some(module) = module.with_peripheral(peripheral).await {
                        final_modules[i] = Some(IdentifiedModule {
                            peripheral: peripheral.clone(),
                            module,
                        });
                    }
                }
            }
        }

        println!("Stopping scan...");
        adapter.stop_scan().await.unwrap();
    }
    Ok(final_modules)
}
