pub mod read;
pub mod write;

use core::time::Duration;

use anyhow::{Context, bail};
use btleplug::api::{Central, CharPropFlags, Characteristic, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Peripheral};
use log::{debug, error, info, trace, warn};
use shared::{Notifier, Writable};
use uuid::Uuid;

/// Peripherals whose advertised local name starts with this are considered ours.
const NAME_PREFIX: &str = "modit";

/// How long to let a scan run before inspecting what it found.
const SCAN_DURATION: Duration = Duration::from_secs(3);

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

/// Deliberately hand-written rather than derived: `Peripheral`'s own `Debug`
/// dumps every property it has cached, which is unreadable in a log line.
impl<T: std::fmt::Debug> std::fmt::Debug for IdentifiedModule<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdentifiedModule")
            .field("peripheral", &self.peripheral.id())
            .field("module", &self.module)
            .finish()
    }
}

/// Parse a UUID, naming the field it came from if it is malformed.
///
/// The raw `uuid::Error` says only that a string was not a UUID, which is not
/// enough to find the typo in a module definition.
fn parse_uuid(value: &str, field: &str) -> anyhow::Result<Uuid> {
    Uuid::parse_str(value).with_context(|| format!("{field} is not a valid UUID: {value:?}"))
}

pub trait ModuleDefinition<T> {
    /// Short name for this kind of module, used in logs and error messages.
    fn label(&self) -> String;

    /// Check every UUID this definition holds.
    ///
    /// Called before any radio work, so a typo fails immediately with a message
    /// naming the module, rather than panicking part-way through a scan.
    fn validate(&self) -> anyhow::Result<()>;

    async fn with_peripheral(&self, peripheral: &Peripheral) -> Option<T>;
}

impl ModuleDefinition<Characteristic> for Notifier {
    fn label(&self) -> String {
        format!("notifier {}", self.service)
    }

    fn validate(&self) -> anyhow::Result<()> {
        parse_uuid(self.service, "service")?;
        parse_uuid(self.charac_notify_id, "charac_notify_id")?;
        Ok(())
    }

    async fn with_peripheral(&self, peripheral: &Peripheral) -> Option<Characteristic> {
        // `validate` has already run for every module, so this cannot normally
        // fail. Log instead of unwrapping so that a caller which skipped
        // validation degrades to "no match" rather than panicking mid-scan.
        let wanted = match parse_uuid(self.charac_notify_id, "charac_notify_id") {
            Ok(uuid) => uuid,
            Err(err) => {
                error!("{}: {err:#}", self.label());
                return None;
            }
        };

        for characteristic in peripheral.characteristics() {
            trace!("checking notifier characteristic {characteristic:?}");
            if characteristic.uuid == wanted
                && characteristic.properties.contains(CharPropFlags::NOTIFY)
            {
                info!("subscribing to characteristic {}", characteristic.uuid);
                if let Err(err) = peripheral.subscribe(&characteristic).await {
                    warn!(
                        "found characteristic {} but could not subscribe: {err}",
                        characteristic.uuid
                    );
                    return None;
                }
                return Some(characteristic);
            }
        }
        None
    }
}

impl ModuleDefinition<Characteristic> for Writable {
    fn label(&self) -> String {
        format!("writable {}", self.service)
    }

    fn validate(&self) -> anyhow::Result<()> {
        parse_uuid(self.service, "service")?;
        parse_uuid(self.charac_write_id, "charac_write_id")?;
        Ok(())
    }

    async fn with_peripheral(&self, peripheral: &Peripheral) -> Option<Characteristic> {
        // See the note in the `Notifier` impl above.
        let wanted = match parse_uuid(self.charac_write_id, "charac_write_id") {
            Ok(uuid) => uuid,
            Err(err) => {
                error!("{}: {err:#}", self.label());
                return None;
            }
        };

        for characteristic in peripheral.characteristics() {
            trace!("checking writable characteristic {characteristic:?}");
            if characteristic.uuid == wanted
                && characteristic.properties.contains(CharPropFlags::WRITE)
            {
                return Some(characteristic);
            }
        }
        None
    }
}

/// Check every module definition before the radio is touched.
pub fn validate_modules<T>(modules: &[impl ModuleDefinition<T>]) -> anyhow::Result<()> {
    if modules.is_empty() {
        bail!("no modules defined: there is nothing for the brain to talk to");
    }
    for (i, module) in modules.iter().enumerate() {
        module
            .validate()
            .with_context(|| format!("module {i} ({})", module.label()))?;
    }
    Ok(())
}

pub async fn init_bluetooth<T>(
    adapter_list: &[Adapter],
    modules: &[impl ModuleDefinition<T>],
) -> anyhow::Result<Vec<Option<IdentifiedModule<T>>>> {
    validate_modules(modules)?;

    if adapter_list.is_empty() {
        bail!(
            "no Bluetooth adapters found. Things to check:\n  \
             - Bluetooth is switched on.\n  \
             - macOS: the terminal running this needs Bluetooth permission, in \
             System Settings > Privacy & Security > Bluetooth.\n  \
             - Linux: the bluetooth service is running (systemctl status bluetooth)."
        );
    }

    let mut final_modules: Vec<Option<IdentifiedModule<T>>> =
        (0..modules.len()).map(|_| None).collect::<Vec<_>>();

    for (adapter_index, adapter) in adapter_list.iter().enumerate() {
        // `Adapter`'s Debug impl dumps every peripheral it has ever seen, which
        // is both unreadable and a way to leak nearby device names into logs.
        let adapter_name = adapter
            .adapter_info()
            .await
            .unwrap_or_else(|_| format!("adapter {adapter_index}"));

        debug!("starting scan on {adapter_name}");
        adapter
            .start_scan(ScanFilter::default())
            .await
            .with_context(|| format!("could not start a BLE scan on {adapter_name}"))?;

        tokio::time::sleep(SCAN_DURATION).await;

        let peripherals = adapter
            .peripherals()
            .await
            .context("could not list the peripherals found by the scan")?;

        if peripherals.is_empty() {
            warn!("scan found no BLE peripherals at all; will rescan");
        } else {
            debug!("scan found {} peripheral(s)", peripherals.len());
            for peripheral in peripherals.iter() {
                // Already claimed by a module in an earlier pass.
                if final_modules
                    .iter()
                    .flatten()
                    .any(|m| m.peripheral.id() == peripheral.id())
                {
                    continue;
                }

                let properties = peripheral
                    .properties()
                    .await
                    .context("could not read peripheral properties")?;
                let local_name = properties.and_then(|p| p.local_name);
                let Some(local_name) = local_name else {
                    trace!("skipping unnamed peripheral {:?}", peripheral.id());
                    continue;
                };
                if !local_name.starts_with(NAME_PREFIX) {
                    trace!("skipping {local_name:?}: not a {NAME_PREFIX}* device");
                    continue;
                }

                info!("found {local_name:?}");

                let is_connected = peripheral.is_connected().await.with_context(|| {
                    format!("could not query connection state of {local_name:?}")
                })?;

                if !is_connected {
                    debug!("connecting to {local_name:?}...");
                    if let Err(err) = peripheral.connect().await {
                        warn!("could not connect to {local_name:?}, skipping it: {err}");
                        continue;
                    }
                    info!("connected to {local_name:?}");
                }

                debug!("discovering services on {local_name:?}...");
                if let Err(err) = peripheral.discover_services().await {
                    warn!("could not discover services on {local_name:?}, skipping it: {err}");
                    continue;
                }

                for (i, module) in modules.iter().enumerate() {
                    if final_modules[i].is_some() {
                        continue;
                    }
                    if let Some(found) = module.with_peripheral(peripheral).await {
                        info!("bound module {i} ({}) to {local_name:?}", module.label());
                        final_modules[i] = Some(IdentifiedModule {
                            peripheral: peripheral.clone(),
                            module: found,
                        });
                    }
                }
            }
        }

        debug!("stopping scan on {adapter_name}");
        if let Err(err) = adapter.stop_scan().await {
            // Not fatal: the scan stops on its own when the adapter is dropped,
            // and we may already have everything we need.
            warn!("could not stop the scan cleanly: {err}");
        }
    }

    Ok(final_modules)
}

/// Names the modules that no peripheral satisfied, for the retry message.
pub fn unbound_labels<T, D: ModuleDefinition<T>>(
    modules: &[D],
    found: &[Option<IdentifiedModule<T>>],
) -> Vec<String> {
    modules
        .iter()
        .enumerate()
        .zip(found)
        .filter(|(_, slot)| slot.is_none())
        .map(|((i, module), _)| format!("module {i} ({})", module.label()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "937312e0-2354-11eb-9f10-fbc30a62cf30";
    // One character short of a UUID.
    const BAD: &str = "937312e0-2354-11eb-9f10-fbc30a62cf3";

    fn notifier(charac: &'static str) -> Notifier {
        Notifier {
            service: GOOD,
            charac_notify_id: charac,
        }
    }

    #[test]
    fn a_valid_definition_passes() {
        validate_modules::<Characteristic>(&[notifier(GOOD)]).unwrap();
    }

    #[test]
    fn a_bad_uuid_names_the_module_the_field_and_the_value() {
        let err = validate_modules::<Characteristic>(&[notifier(GOOD), notifier(BAD)])
            .expect_err("a malformed UUID must not validate");

        // `{:#}` renders the whole anyhow context chain, which is what the user
        // actually sees at the top level.
        let rendered = format!("{err:#}");
        assert!(
            rendered.contains("module 1"),
            "should name which module: {rendered}"
        );
        assert!(
            rendered.contains("charac_notify_id"),
            "should name which field: {rendered}"
        );
        assert!(
            rendered.contains(BAD),
            "should quote the bad value: {rendered}"
        );
    }

    #[test]
    fn an_empty_module_list_is_an_error() {
        let err = validate_modules::<Characteristic>(&[] as &[Notifier])
            .expect_err("no modules is a configuration mistake, not a valid setup");
        assert!(format!("{err:#}").contains("no modules"));
    }

    /// Exercises the no-adapter branch without having to switch Bluetooth off:
    /// an empty adapter list is the same state `manager.adapters()` reports.
    #[tokio::test]
    async fn no_adapters_explains_what_to_check() {
        let err = init_bluetooth::<Characteristic>(&[], &[notifier(GOOD)])
            .await
            .expect_err("no adapters must be an error, not an empty result");

        let rendered = format!("{err:#}");
        assert!(rendered.contains("no Bluetooth adapters found"));
        // The message has to say what to do about it, not just what happened.
        assert!(rendered.contains("macOS"), "{rendered}");
        assert!(rendered.contains("Linux"), "{rendered}");
    }

    #[test]
    fn a_writable_reports_its_own_field_name() {
        let writable = Writable {
            service: GOOD,
            charac_write_id: BAD,
        };
        let err = validate_modules::<Characteristic>(&[writable]).unwrap_err();
        assert!(format!("{err:#}").contains("charac_write_id"));
    }
}
