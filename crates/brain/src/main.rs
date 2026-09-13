mod ble;
mod link;
mod runtime;
mod scenarios;

use anyhow::Context;
use ble::*;
use btleplug::api::{Characteristic, Manager as _};
use btleplug::platform::Manager;
use link::{ModuleId, sim, sim::SimLink};
use log::{info, warn};
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

/// Which physical boards a scenario expects on the bench.
///
/// Introduced with the coin module: until now every scenario wanted the same
/// two buttons, so the module list could be a constant. A scenario that needs a
/// coin acceptor *and* buttons cannot be described that way, and neither can the
/// next one that needs a keypad and a maglock.
struct Bench {
    buttons: Vec<&'static str>,
    /// The id of the coin acceptor, if this scenario takes money.
    coin: Option<&'static str>,
}

impl Bench {
    fn for_scenario(name: &str) -> Self {
        match name {
            // A coin slot plus the same two buttons the other scenarios use.
            "arcade" => Self {
                buttons: vec!["a", "b"],
                coin: Some("slot"),
            },
            _ => Self {
                buttons: vec!["a", "b"],
                coin: None,
            },
        }
    }

    /// The simulated bench: each module paired with what it claims to be.
    ///
    /// The coin slot comes first so that `arcade`'s attract mode is the first
    /// thing the log shows, rather than being buried under two button hellos.
    fn sim_specs(&self) -> Vec<(ModuleId, shared::proto::Descriptor)> {
        let coin = self.coin.map(|id| (ModuleId(id.to_string()), sim::coin()));
        let buttons = self
            .buttons
            .iter()
            .map(|id| (ModuleId(id.to_string()), sim::button(1, 1)));
        coin.into_iter().chain(buttons).collect()
    }

    /// The BLE definitions, which today can only be buttons.
    fn button_modules(&self) -> Vec<ButtonLed> {
        self.buttons.iter().copied().map(button_module).collect()
    }
}

/// Read the `--modules` list: the ids of the boards actually on the bench.
///
/// The default bench is two buttons, because that is what every scenario here
/// was written against. Someone who has built *one* board should still be able
/// to play, and today they cannot: acquisition refuses to start until every
/// module in the bench is bound, so a missing `b` means an endless rescan with
/// no hint that the list is the thing to change.
///
/// Ids are `&'static str` because the id is baked into a board at flash time and
/// names that board for as long as the process runs. Command-line strings are
/// not static, so they are leaked -- a handful of short strings, once, at
/// startup, that would live until exit regardless.
fn parse_modules(list: &str) -> anyhow::Result<Vec<&'static str>> {
    let mut ids: Vec<&str> = Vec::new();
    for id in list.split(',') {
        let id = id.trim();
        if id.is_empty() {
            anyhow::bail!(
                "{list:?} has an empty id. Write the list as `--modules a,b`, \
                 matching the ids the boards were flashed with."
            );
        }
        if ids.contains(&id) {
            anyhow::bail!(
                "{id:?} is listed twice. Two boards flashed with the same id \
                 advertise the same name, and only one of them can be bound."
            );
        }
        ids.push(id);
    }
    Ok(ids
        .into_iter()
        .map(|id| &*String::leak(id.to_string()))
        .collect())
}

const USAGE: &str = "\
modit brain -- runs a scenario against a set of modules

USAGE:
    brain [--simulate] [--scenario <name>] [--modules <ids>]

OPTIONS:
    --simulate           Run with no radio and no boards. Presses come from the
                         keyboard. Good for trying a scenario on a train.
    --modules <ids>      Comma-separated ids of the boards on the bench,
                         matching what each was flashed with. Defaults to `a,b`.
                         With one board, use `--modules a`.
    --scenario <name>    which game to run:
                           whack      light one at random, press it, repeat
                                      forever (the default)
                           simon      repeat a growing sequence from memory
                           speedrun   10 timed laps, reports every split
                           arcade     pay a coin, play a round (needs a coin
                                      acceptor -- simulation only for now)
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
    // Timed, because the questions a game log has to answer are all about when:
    // whether a round was hit instantly or after a pause, whether a module went
    // quiet before or after the adapter did. Without timestamps a runaway loop
    // and someone playing well look identical afterwards.
    pretty_env_logger::formatted_timed_builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let simulate = args.iter().any(|arg| arg == "--simulate");
    let scenario_name = match args.iter().position(|a| a == "--scenario") {
        Some(i) => args.get(i + 1).map(String::as_str).unwrap_or(""),
        None => "whack",
    };
    // Boxed so both scenarios share one type; they are still plain async fns.
    let scenario: Scenario = match scenario_name {
        "whack" => |m| Box::pin(scenarios::whack_a_mole(m)) as _,
        "simon" => |m| Box::pin(scenarios::simon_says(m)) as _,
        "speedrun" => |m| Box::pin(scenarios::speedrun(m)) as _,
        "arcade" => |m| Box::pin(scenarios::arcade(m)) as _,
        other => anyhow::bail!("unknown scenario {other:?}. Known: whack, simon, speedrun, arcade"),
    };

    let mut bench = Bench::for_scenario(scenario_name);
    if let Some(i) = args.iter().position(|a| a == "--modules") {
        let list = args
            .get(i + 1)
            .context("--modules needs a list of ids, for example `--modules a`")?;
        bench.buttons = parse_modules(list).context("invalid --modules")?;
    }

    if simulate {
        info!("running in simulation -- no radio, no boards");
        let (link, handle) = SimLink::new(bench.sim_specs());
        let coin_slot = ModuleId(bench.coin.unwrap_or("slot").to_string());
        let keyboard = tokio::spawn(sim_keyboard(handle, coin_slot));
        let result = runtime::run(link, scenario).await;
        keyboard.abort();
        return result;
    }

    // The BLE link is still typed to button modules, because that firmware
    // speaks its own wire format rather than shared::proto. Say so plainly
    // rather than scanning for a board that could not be driven anyway.
    if bench.coin.is_some() {
        anyhow::bail!(
            "the {scenario_name:?} scenario needs a coin acceptor, and the BLE link \
             cannot drive one: it still speaks the button firmware's wire format. \
             Run it with --simulate."
        );
    }
    let modules = bench.button_modules();

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
async fn sim_keyboard(handle: link::sim::SimHandle, coin_slot: ModuleId) {
    use tokio::io::{AsyncBufReadExt, BufReader};

    info!("simulator ready:");
    info!("  a       press module a's button");
    info!("  $       post a 1-pulse coin into the slot");
    info!("  $3      post a coin the acceptor reads as 3 pulses");
    info!("  -a      make module a drop out, as if it lost power");
    info!("  q       quit");
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        match line.as_str() {
            "q" => std::process::exit(0),
            "" => continue,
            // `$` rather than `c`, so it cannot collide with a module id.
            coin if coin.starts_with('$') => {
                let pulses = match coin[1..].trim() {
                    "" => 1,
                    n => match n.parse::<u8>() {
                        Ok(pulses) => pulses,
                        Err(_) => {
                            warn!("{n:?} is not a pulse count; try `$` or `$3`");
                            continue;
                        }
                    },
                };
                info!("[sim] posting a {pulses}-pulse coin");
                handle.insert_coin(&coin_slot, pulses);
            }
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

    /// One board is a legitimate bench, and the commonest one while building:
    /// you flash `a`, wire it, and want to see it work before soldering `b`.
    #[test]
    fn a_single_module_is_a_valid_bench() {
        assert_eq!(parse_modules("a").unwrap(), vec!["a"]);
    }

    #[test]
    fn modules_are_split_and_trimmed() {
        assert_eq!(parse_modules("a, b ,c").unwrap(), vec!["a", "b", "c"]);
    }

    /// Both of these would otherwise fail much later and much less clearly: an
    /// empty id never matches a board, and a duplicate binds once and then waits
    /// forever for a second board that is really the same one.
    #[test]
    fn an_empty_or_duplicated_id_is_refused() {
        assert!(parse_modules("a,,b").is_err());
        assert!(parse_modules("a,a").is_err());
    }

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

    /// How one kind of board spells a pin: in a net table, and in firmware.
    struct PinSpelling {
        /// Precedes the pin number in a net table row, e.g. `` `GPIO `` in `` `GPIO27` ``.
        table: &'static str,
        /// Precedes the pin number where the firmware takes the pin.
        firmware: &'static str,
    }

    const ESP32: PinSpelling = PinSpelling {
        table: "`GPIO",
        firmware: "peripherals.GPIO",
    };
    const UNO: PinSpelling = PinSpelling {
        table: "`D",
        firmware: "pins.d",
    };

    /// Which documentation page describes which firmware, and on which board.
    /// One entry per firmware; a page may carry a table for several boards.
    const WIRING_DOCS: &[(&str, &str, PinSpelling)] = &[
        (
            "/../../docs/hardware/module-button.md",
            "/../module-button/src/main.rs",
            ESP32,
        ),
        (
            "/../../docs/hardware/module-coin.md",
            "/../module-coin/src/main.rs",
            ESP32,
        ),
        (
            "/../../docs/hardware/module-coin.md",
            "/../module-coin-uno/src/main.rs",
            UNO,
        ),
    ];

    /// Every number directly following `prefix` in `text`, sorted and
    /// deduplicated.
    fn pin_numbers(text: &str, prefix: &str) -> Vec<u32> {
        let mut found: Vec<u32> = text
            .match_indices(prefix)
            .filter_map(|(at, _)| {
                let digits: String = text[at + prefix.len()..]
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect();
                digits.parse().ok()
            })
            .collect();
        found.sort_unstable();
        found.dedup();
        found
    }

    /// The net tables in `docs/hardware/` must name the same pins the firmware
    /// actually uses.
    ///
    /// Documentation that describes wiring is the kind that goes stale in
    /// silence: nothing fails, a board is simply built wrong months later. Same
    /// reasoning as `firmware_uuids_match_shared` above -- if the two sources
    /// cannot be unified, at least make a divergence loud.
    ///
    /// Only the **net table** is read, not the whole page, so prose is free to
    /// mention other pins (`docs/HARDWARE.md` discusses the strapping pins, and
    /// should keep being able to).
    #[test]
    fn wiring_tables_match_firmware() {
        for (doc_rel, fw_rel, spelling) in WIRING_DOCS {
            let doc_path = format!("{}{doc_rel}", env!("CARGO_MANIFEST_DIR"));
            let fw_path = format!("{}{fw_rel}", env!("CARGO_MANIFEST_DIR"));

            let doc = std::fs::read_to_string(&doc_path)
                .unwrap_or_else(|e| panic!("cannot read {doc_path}: {e}"));
            let firmware = std::fs::read_to_string(&fw_path)
                .unwrap_or_else(|e| panic!("cannot read {fw_path}: {e}"));

            // The net table is the table rows naming a pin in this board's
            // spelling. Anything else on the page is prose, or another board.
            let table: String = doc
                .lines()
                .filter(|line| line.starts_with('|'))
                .filter(|line| line.contains(spelling.table))
                .collect::<Vec<_>>()
                .join("\n");
            let documented = pin_numbers(&table, spelling.table);
            assert!(
                !documented.is_empty(),
                "{doc_path} has no net table rows naming a `{}nn` pin. \
                 If the table changed shape, update this test.",
                spelling.table.trim_start_matches('`')
            );

            // The firmware's pins are whatever it asks the HAL for.
            let used = pin_numbers(&firmware, spelling.firmware);
            assert!(
                !used.is_empty(),
                "{fw_path} never mentions `{}nn`. \
                 If the firmware changed shape, update this test.",
                spelling.firmware
            );

            assert_eq!(
                documented, used,
                "the net table in {doc_path} and the pins used by {fw_path} \
                 have drifted apart. The table is the source of truth for a \
                 human wiring a board -- fix whichever is wrong, and remember \
                 the schematic (`just schematics`) may need regenerating too."
            );
        }
    }

    /// The capability table in `docs/COMPOSING.md`, and what each row can be
    /// checked against.
    ///
    /// Each entry is the module's name, its hardware page, its firmware, and
    /// whether that firmware declares a `Descriptor`.
    ///
    /// `false` means it does not have one *yet*: `module-button` still speaks the
    /// older button-specific wire format. The test below asserts that absence
    /// rather than skipping quietly, so the day that firmware grows a descriptor
    /// the stronger check is wired up rather than forgotten.
    const CAPABILITIES: &[(&str, &str, &str, bool)] = &[
        (
            "module-button",
            "/../../docs/hardware/module-button.md",
            "/../module-button/src/main.rs",
            false,
        ),
        (
            "module-coin",
            "/../../docs/hardware/module-coin.md",
            "/../module-coin/src/main.rs",
            true,
        ),
    ];

    /// One row of the capability table.
    #[derive(Debug)]
    struct Capability {
        module: String,
        role: String,
        inputs: usize,
        outputs: usize,
    }

    /// Read the capability table out of `docs/COMPOSING.md`.
    ///
    /// A row is a table line whose first cell links to a module's hardware page,
    /// which is also what makes the table self-describing: a row with no page is
    /// a claim with no wiring behind it.
    fn capability_table() -> Vec<Capability> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/COMPOSING.md");
        let page =
            std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));

        let rows: Vec<Capability> = page
            .lines()
            .filter(|line| line.starts_with('|') && line.contains("](hardware/module-"))
            .map(|line| {
                let cells: Vec<&str> = line.split('|').map(str::trim).collect();
                assert!(
                    cells.len() >= 6,
                    "the capability row {line:?} does not have the expected \
                     columns (module, role, inputs, outputs, channels). If the \
                     table changed shape, update this test."
                );
                let module = cells[1]
                    .split_once('[')
                    .and_then(|(_, rest)| rest.split_once(']'))
                    .map(|(name, _)| name.to_string())
                    .unwrap_or_else(|| panic!("no module name in {:?}", cells[1]));
                let count = |cell: &str, what: &str| -> usize {
                    cell.parse().unwrap_or_else(|_| {
                        panic!("{module}'s {what} column is {cell:?}, not a number")
                    })
                };
                Capability {
                    role: cells[2].trim_matches('`').to_string(),
                    inputs: count(cells[3], "inputs"),
                    outputs: count(cells[4], "outputs"),
                    module,
                }
            })
            .collect();

        assert!(
            !rows.is_empty(),
            "{path} has no capability rows linking to a hardware page. \
             If the table changed shape, update this test."
        );
        rows
    }

    /// Count the input and output nets on a module's hardware page.
    ///
    /// The net table is already the source of truth for the pins, and
    /// `wiring_tables_match_firmware` above keeps it honest against the
    /// firmware. Counting its rows therefore reaches the firmware's real channel
    /// count without this test having to parse a `Descriptor` the firmware may
    /// not declare yet.
    fn nets(page: &str) -> (usize, usize) {
        let directions = page
            .lines()
            .filter(|line| line.starts_with('|') && line.contains("`GPIO"))
            .filter_map(|line| line.split('|').nth(3).map(str::trim));
        let mut inputs = 0;
        let mut outputs = 0;
        for direction in directions {
            if direction.starts_with("input") {
                inputs += 1;
            } else if direction.starts_with("output") {
                outputs += 1;
            }
            // Anything else -- `analog in`, a power rail -- is not a channel a
            // scenario can address, so it is not counted.
        }
        (inputs, outputs)
    }

    /// Pull `role`, `inputs` and `outputs` out of a firmware's `DESCRIPTOR`.
    ///
    /// String matching for the same reason as `firmware_uuids_match_shared`: the
    /// firmware crates are separate workspaces built for another target, so this
    /// test cannot link them.
    fn firmware_descriptor(source: &str) -> Option<(String, usize, usize)> {
        let body = source
            .split_once("const DESCRIPTOR: Descriptor = Descriptor {")?
            .1
            .split_once('}')?
            .0;
        let field = |name: &str| -> Option<&str> {
            body.lines()
                .map(str::trim)
                .find_map(|line| line.strip_prefix(name)?.trim().strip_suffix(','))
        };
        let number = |name: &str| -> usize {
            field(name)
                .unwrap_or_else(|| panic!("no `{name}` in the firmware DESCRIPTOR"))
                .parse()
                .expect("a channel count in the firmware DESCRIPTOR")
        };
        let role = field("role:")
            .and_then(|value| value.strip_prefix("Role::"))
            .expect("a `role: Role::..` line in the firmware DESCRIPTOR")
            .to_string();
        Some((role, number("inputs:"), number("outputs:")))
    }

    /// The capability table in `docs/COMPOSING.md` must describe the modules that
    /// actually exist.
    ///
    /// Three things are checked, because the table makes three different claims:
    ///
    /// 1. every row names a module this test knows about, so a row cannot be
    ///    added without deciding what verifies it;
    /// 2. the channel counts match the module's net table, which
    ///    `wiring_tables_match_firmware` already ties to the firmware's pins;
    /// 3. where the firmware declares a `Descriptor`, the row matches that too --
    ///    role included, which no net table knows about.
    ///
    /// Plus the reverse direction: every [`shared::proto::Role`] has a row. A
    /// module type the documentation never mentions is one nobody can design a
    /// game around, which is the whole purpose of that page.
    #[test]
    fn capability_table_matches_the_modules() {
        let rows = capability_table();

        for row in &rows {
            let (_, doc_rel, fw_rel, declares_descriptor) = CAPABILITIES
                .iter()
                .find(|(name, _, _, _)| *name == row.module)
                .unwrap_or_else(|| {
                    panic!(
                        "the capability table has a row for {}, which this test \
                         does not know how to check. Add it to CAPABILITIES with \
                         its hardware page and firmware -- a documented module \
                         with nothing to check the row against is how that table \
                         starts describing something nobody built.",
                        row.module
                    )
                });

            let doc_path = format!("{}{doc_rel}", env!("CARGO_MANIFEST_DIR"));
            let page = std::fs::read_to_string(&doc_path)
                .unwrap_or_else(|e| panic!("cannot read {doc_path}: {e}"));
            assert_eq!(
                (row.inputs, row.outputs),
                nets(&page),
                "{} is documented as {} input(s) and {} output(s) in \
                 docs/COMPOSING.md, but its net table in {doc_path} names a \
                 different number of channels. The net table is the source of \
                 truth -- fix the capability table to agree with it.",
                row.module,
                row.inputs,
                row.outputs
            );

            let fw_path = format!("{}{fw_rel}", env!("CARGO_MANIFEST_DIR"));
            let firmware = std::fs::read_to_string(&fw_path)
                .unwrap_or_else(|e| panic!("cannot read {fw_path}: {e}"));
            let declared = firmware_descriptor(&firmware);

            if !declares_descriptor {
                assert!(
                    declared.is_none(),
                    "{fw_path} now declares a DESCRIPTOR. Flip {}'s entry in \
                     CAPABILITIES to `true`, so the documented role and channel \
                     counts are checked against the firmware and not only \
                     against the net table.",
                    row.module
                );
                continue;
            }

            let (role, inputs, outputs) = declared.unwrap_or_else(|| {
                panic!(
                    "{fw_path} has no `const DESCRIPTOR: Descriptor = Descriptor {{`. \
                     If the firmware changed shape, update this test."
                )
            });
            assert_eq!(
                (row.role.as_str(), row.inputs, row.outputs),
                (role.as_str(), inputs, outputs),
                "the capability table says {} is a {} with {}/{} channels, but \
                 {}{fw_rel} reports a {role} with {inputs}/{outputs}",
                row.module,
                row.role,
                row.inputs,
                row.outputs,
                env!("CARGO_MANIFEST_DIR"),
            );
        }

        for role in shared::proto::Role::ALL {
            assert!(
                rows.iter().any(|row| row.role == role.name()),
                "no row in the docs/COMPOSING.md capability table describes the \
                 {} role. A module type that page does not mention is one nobody \
                 can design a game around.",
                role.name()
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
