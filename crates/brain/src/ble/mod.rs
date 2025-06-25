pub mod notifier;
pub mod writable;

use core::time;
use core::time::Duration;

use crate::logic::{CoinReceiver, MotorController};
use async_trait::async_trait;
use btleplug::api::{
    Central, CharPropFlags, Characteristic, Manager as _, Peripheral as _, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::stream::StreamExt;
use shared::Notifier;
use uuid::Uuid;

/// Only devices whose name contains this string will be tried.
const PERIPHERAL_NAME_MATCH_FILTER: &str = "Modit";
/// UUID of the characteristic for which we should subscribe to notifications.
const NOTIFY_CHARACTERISTIC_UUID: Uuid = Uuid::from_u128(0x00002a1900001000800000805f9b34fb); // 2ab4

#[derive(Clone, Debug)]
pub enum Module {
    Notifier(Notifier),
    Writable(Writable),
}

pub async fn init_bluetooth(modules: &[Module]) -> anyhow::Result<Vec> {
    let manager = Manager::new().await?;
    let adapter_list = manager.adapters().await?;
    if adapter_list.is_empty() {
        eprintln!("No Bluetooth adapters found");
    }

    let mut peripherals = vec![];
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
            let mut wanted_modules = modules.to_vec();
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
                for module in wanted_modules.clone() {
                    // Check if it's the peripheral we want.
                    match module {
                        Module::Notifier(Notifier {
                            service,
                            charac_notify_id,
                        }) => {
                            if local_name == mo {
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
                                        if characteristic.uuid == charac_notify_id
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
                                    }
                                }
                                break;
                            } else {
                                //println!("Skipping unknown peripheral {:?}", peripheral);
                            }
                        }
                        Module::Writable(Writable {
                            service,
                            charac_notify_id,
                        }) => {
                            todo!()
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
