mod ble;
mod link;
mod runtime;
mod scenarios;
#[cfg(test)]
mod scenarios_tests;

use anyhow::Context;
use ble::*;
use btleplug::api::{Characteristic, Manager as _};
use btleplug::platform::Manager;
use link::{ModuleId, sim::SimLink};
use log::info;
use shared::{Notifier, Writable, uuids};

#[derive(Clone, Debug)]
pub struct ButtonLed {
    /// Which physical board this is. Must match the `MODIT_ID` it was flashed
    /// with, so that a module is the same box on every run.
    pub id: &'static str,
    pub button: Notifier,
    pub led: Writable,
}

#[derive(Clone, Debug)]
pub struct ButtonDetails {
    pub button: Characteristic,
    pub led: Characteristic,
}

impl Module<ButtonDetails> for ButtonLed {
    fn advertised_name(&self) -> String {
        format!("modit-button-{}", self.id)
    }
}

impl ModuleDefinition<ButtonDetails> for ButtonLed {
    fn label(&self) -> String {
        format!("button-led {}", self.id)
    }

    fn validate(&self) -> anyhow::Result<()> {
        self.button.validate().context("button")?;
        self.led.validate().context("led")?;
        Ok(())
    }

    async fn with_peripheral(
        &self,
        peripheral: &btleplug::platform::Peripheral,
    ) -> Option<ButtonDetails> {
        let button = (self.button.with_peripheral(peripheral).await)?;
        let led = (self.led.with_peripheral(peripheral).await)?;
        Some(ButtonDetails { button, led })
    }
}

/// One entry per physical board. The id must match what the board was flashed
/// with: `just flash a` produces the board the first entry expects.
fn button_module(id: &'static str) -> ButtonLed {
    ButtonLed {
        id,
        button: Notifier {
            service: uuids::SERVICE,
            charac_notify_id: uuids::BUTTON_NOTIFY,
        },
        led: Writable {
            service: uuids::SERVICE,
            charac_write_id: uuids::LED_WRITE,
        },
    }
}

const USAGE: &str = "\
modit brain -- runs a scenario against a set of modules

USAGE:
    brain [--simulate] [--scenario <name>]

OPTIONS:
    --simulate           Run with no radio and no boards. Presses come from the
                         keyboard. Good for trying a scenario on a train.
    --scenario <name>    whack (default) or simon
    -h, --help           Show this

ENVIRONMENT:
    RUST_LOG             warn | info (default) | brain=debug | brain=trace
";

/// A scenario is just an async fn over `Modules` -- no trait, because neither
/// scenario needs a lifecycle hook.
type Scenario =
    fn(
        std::sync::Arc<runtime::Modules>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send>>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Default to `info` so a bare `cargo run` is useful, but let RUST_LOG win.
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let simulate = args.iter().any(|arg| arg == "--simulate");
    let scenario = match args.iter().position(|a| a == "--scenario") {
        Some(i) => args.get(i + 1).map(String::as_str).unwrap_or(""),
        None => "whack",
    };
    // Boxed so both scenarios share one type; they are still plain async fns.
    let scenario: Scenario = match scenario {
        "whack" => |m| Box::pin(scenarios::whack_a_mole(m)) as _,
        "simon" => |m| Box::pin(scenarios::simon_says(m)) as _,
        other => anyhow::bail!("unknown scenario {other:?}. Known: whack, simon"),
    };

    let modules = vec![button_module("a"), button_module("b")];

    if simulate {
        info!("running in simulation -- no radio, no boards");
        let ids: Vec<ModuleId> = modules.iter().map(|m| ModuleId(m.id.to_string())).collect();
        let (link, handle) = SimLink::new(ids, 1, 1);
        let keyboard = tokio::spawn(sim_keyboard(handle));
        let result = runtime::run(link, scenario).await;
        keyboard.abort();
        return result;
    }

    // Fail on a bad definition before touching the radio, so a typo is reported
    // as a typo rather than as a scan that never finds anything.
    validate_modules(&modules).context("invalid module definition")?;

    let manager = Manager::new()
        .await
        .context("could not initialise the Bluetooth manager")?;
    let adapter_list = manager
        .adapters()
        .await
        .context("could not list Bluetooth adapters")?;
    info!("{} Bluetooth adapter(s) available", adapter_list.len());

    let link = link::ble::BleLink::new(adapter_list, modules);
    runtime::run(link, scenario).await
}

/// Turn keystrokes into button presses, so a scenario can be played without
/// hardware. Type a module id and press enter.
async fn sim_keyboard(handle: link::sim::SimHandle) {
    use tokio::io::{AsyncBufReadExt, BufReader};

    info!("simulator ready:");
    info!("  a       press module a's button");
    info!("  -a      make module a drop out, as if it lost power");
    info!("  q       quit");
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        match line.as_str() {
            "q" => std::process::exit(0),
            "" => continue,
            dropped if dropped.starts_with('-') => {
                let id = ModuleId(dropped[1..].to_string());
                info!("[sim] {id} drops out");
                handle.lose(&id);
            }
            id => handle.press(&ModuleId(id.to_string()), 0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The firmware cannot reference `shared::uuids` -- `gatt!` parses UUIDs when
    /// it expands and only accepts string literals -- so the two are duplicated.
    /// This is what stops them drifting apart, which is exactly what happened to
    /// the old `assets/` files.
    ///
    /// Reads the firmware source rather than linking it, because that crate is
    /// built for a different target with a different toolchain.
    #[test]
    fn firmware_uuids_match_shared() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../module-button/src/main.rs");
        let src = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("cannot read the firmware source at {path}: {e}"));

        let found: Vec<&str> = src
            .lines()
            .filter_map(|line| {
                let rest = line.trim().strip_prefix("uuid: \"")?;
                rest.split('"').next()
            })
            .collect();

        let expected = [uuids::SERVICE, uuids::LED_WRITE, uuids::BUTTON_NOTIFY];
        assert_eq!(
            found.len(),
            expected.len(),
            "expected {} uuid literals in the firmware, found {}: {found:?}. \
             If the gatt! block changed shape, update this test.",
            expected.len(),
            found.len()
        );
        for (found, expected) in found.iter().zip(expected) {
            assert_eq!(
                *found, expected,
                "the firmware and shared::uuids have drifted apart"
            );
        }
    }

    /// `shared` cannot check this itself: it has no UUID parser, by design.
    #[test]
    fn every_well_known_uuid_parses() {
        for (name, value) in [
            ("SERVICE", uuids::SERVICE),
            ("LED_WRITE", uuids::LED_WRITE),
            ("BUTTON_NOTIFY", uuids::BUTTON_NOTIFY),
        ] {
            uuid::Uuid::parse_str(value)
                .unwrap_or_else(|e| panic!("uuids::{name} is not a valid UUID: {e}"));
        }
    }

    #[test]
    fn a_module_advertises_the_name_its_board_was_flashed_with() {
        // Must match `modit-{MODIT_ROLE}-{MODIT_ID}` in the firmware.
        assert_eq!(button_module("a").advertised_name(), "modit-button-a");
    }
}
