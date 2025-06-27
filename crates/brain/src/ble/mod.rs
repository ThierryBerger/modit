pub mod notifier;
pub mod writable;

use core::time::Duration;
use std::collections::HashMap;

use btleplug::api::{
    Central, CharPropFlags, Characteristic, Manager as _, Peripheral as _, ScanFilter,
};
use btleplug::platform::{Manager, Peripheral};
use futures::stream::StreamExt;
use shared::{Notifier, Writable};
use uuid::Uuid;

/// Only devices whose name contains this string will be tried.
const PERIPHERAL_NAME_MATCH_FILTER: &str = "Modit";
/// UUID of the characteristic for which we should subscribe to notifications.
const NOTIFY_CHARACTERISTIC_UUID: Uuid = Uuid::from_u128(0x00002a1900001000800000805f9b34fb); // 2ab4

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum Module {
    Notifier(Notifier),
    Writable(Writable),
}

pub async fn init_bluetooth(
    modules: &[Module],
) -> anyhow::Result<HashMap<Module, (Peripheral, Characteristic)>> {
    let manager = Manager::new().await?;
    let adapter_list = manager.adapters().await?;
    if adapter_list.is_empty() {
        eprintln!("No Bluetooth adapters found");
    }

    let mut final_modules = HashMap::<Module, (Peripheral, Characteristic)>::new();
    let mut modules_left_to_initialize = modules.clone().to_vec();
    for adapter in dbg!(adapter_list).iter() {
        println!("Starting scan...");
        adapter
            .start_scan(ScanFilter::default())
            .await
            .expect("Can't scan BLE adapter for connected devices...");
        tokio::time::sleep(Duration::from_secs(3)).await;
        let peripherals = adapter.peripherals().await?;
        // Remove modules already initialized
        modules_left_to_initialize.retain(|module| !final_modules.keys().any(|key| key == module));
        if peripherals.is_empty() {
            eprintln!("->>> BLE peripheral devices were not found, sorry. Exiting...");
        } else {
            // All peripheral devices in range.
            for peripheral in peripherals.iter() {
                let properties = peripheral.properties().await?;
                let is_connected = peripheral.is_connected().await?;
                let local_name = properties
                    .unwrap()
                    .local_name
                    .unwrap_or(String::from("(peripheral name unknown)"));
                println!(
                    "Peripheral {:?} is connected: {:?}",
                    &local_name, is_connected
                );
                /*println!(
                    "Peripheral {:?} is connected: {:?}",
                    &local_name, is_connected
                );*/
                for module in &modules_left_to_initialize {
                    // Check if it's the peripheral we want.
                    match module {
                        Module::Notifier(Notifier {
                            service,
                            charac_notify_id,
                        }) => {
                            if local_name.starts_with("modit") {
                                println!("Found matching peripheral {:?}...", &local_name);
                                if !is_connected {
                                    println!("Trying to connect...");
                                    // Connect if we aren't already connected.
                                    if let Err(err) = peripheral.connect().await {
                                        eprintln!(
                                            "Error connecting to peripheral, skipping: {}",
                                            err
                                        );
                                        continue;
                                    }
                                    println!("Connection has reported success.");
                                }

                                let is_connected = peripheral.is_connected().await?;
                                println!(
                                    "Now connected ({:?}) to peripheral {:?}.",
                                    is_connected, &local_name
                                );
                                if is_connected {
                                    println!("Discover peripheral {:?} services...", local_name);
                                    peripheral.discover_services().await?;
                                    for characteristic in peripheral.characteristics() {
                                        println!("Checking characteristic {:?}", characteristic);
                                        // Subscribe to notifications from the characteristic with the selected
                                        // UUID.
                                        if characteristic.uuid
                                            == Uuid::parse_str(charac_notify_id).unwrap()
                                            && characteristic
                                                .properties
                                                .contains(CharPropFlags::NOTIFY)
                                        {
                                            println!(
                                                "Subscribing to characteristic {:?}",
                                                characteristic.uuid
                                            );
                                            peripheral.subscribe(&characteristic).await?;
                                        }
                                        final_modules.insert(
                                            module.clone(),
                                            (peripheral.clone(), characteristic.clone()),
                                        );
                                        break;
                                    }
                                }
                                break;
                            } else {
                                //println!("Skipping unknown peripheral {:?}", peripheral);
                            }
                        }
                        Module::Writable(Writable {
                            service,
                            charac_write_id,
                        }) => {
                            todo!()
                        }
                    }
                }
            }
        }
    }
    Ok(final_modules)
}
