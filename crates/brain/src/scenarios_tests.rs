//! Scenario tests, run against the simulated link.
//!
//! These are the first tests in this project that exercise *game logic* rather
//! than parsing or arithmetic. Everything they touch -- the runtime, the
//! scenario, the protocol messages -- is the same code the BLE path runs; only
//! the transport is swapped.

use std::time::Duration;

use crate::link::ModuleId;
use crate::link::sim::{self, SimHandle, SimLink};
use crate::runtime;
use crate::scenarios;

/// Wait for a condition to become true, or give up.
///
/// Every test here runs with `start_paused = true`, so `tokio::time` is virtual:
/// when every task is idle, the clock jumps to the next deadline. That means the
/// scenario's real sleeps cost nothing, and -- more importantly -- these tests do
/// not depend on wall-clock timing, so they cannot go flaky when the machine is
/// busy. Deadlines below are in virtual time.
async fn eventually<F: FnMut() -> bool>(mut condition: F, what: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        if condition() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("timed out waiting for {what}");
}

fn start(ids: &[&str]) -> (SimHandle, tokio::task::JoinHandle<anyhow::Result<()>>) {
    start_with(ids, scenarios::whack_a_mole)
}

fn start_with<S, F>(
    ids: &[&str],
    scenario: S,
) -> (SimHandle, tokio::task::JoinHandle<anyhow::Result<()>>)
where
    S: FnMut(std::sync::Arc<runtime::Modules>) -> F + Send + 'static,
    F: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
{
    let specs = ids
        .iter()
        .map(|id| (ModuleId(id.to_string()), sim::button(1, 1)))
        .collect();
    let (link, handle) = SimLink::new(specs);
    let task = tokio::spawn(runtime::run(link.quiet(), scenario));
    (handle, task)
}

#[tokio::test(start_paused = true)]
async fn exactly_one_module_lights_up() {
    let (sim, task) = start(&["a", "b"]);
    eventually(|| sim.only_lit(0).is_some(), "a module to light up").await;
    task.abort();
}

#[tokio::test(start_paused = true)]
async fn pressing_the_lit_module_moves_the_light() {
    let (sim, task) = start(&["a", "b"]);

    eventually(|| sim.only_lit(0).is_some(), "the first module to light").await;
    let first = sim.only_lit(0).unwrap();

    sim.press(&first, 0);

    // The lit module goes dark...
    eventually(
        || sim.output(&first, 0) == Some(false),
        "the pressed module to go dark",
    )
    .await;
    // ...and something lights up again, ready for the next round.
    eventually(|| sim.only_lit(0).is_some(), "the next module to light").await;

    task.abort();
}

/// Whack-a-mole on one board is a reaction timer rather than a hunt, but it is
/// a game and it is what someone who has soldered exactly one module has. The
/// scenario never assumed two -- only the default bench did, which is what
/// `--modules` exists to override.
#[tokio::test(start_paused = true)]
async fn one_module_is_enough_to_play() {
    let (sim, task) = start(&["a"]);
    let only = ModuleId::from("a");

    eventually(|| sim.output(&only, 0) == Some(true), "the module to light").await;
    sim.press(&only, 0);
    eventually(
        || sim.output(&only, 0) == Some(false),
        "the module to go dark after being pressed",
    )
    .await;
    // And the round comes round again, rather than the scenario ending.
    eventually(
        || sim.output(&only, 0) == Some(true),
        "the module to light for the next round",
    )
    .await;

    task.abort();
}

/// The phantom hit: a light appearing and vanishing instantly with nobody
/// touching it.
///
/// A press queued while the game is between rounds used to be spent on the next
/// target the moment it lit. One board makes it deterministic -- the next target
/// is always the same module, so a stale press always lands on it.
///
/// Counts rounds from the history rather than polling for the light, for the
/// reason `arcade::rounds_finished` gives: a phantom round opens and closes
/// between two polls, so watching the LED can miss it entirely.
///
/// The firmware half of this is `EdgeLatch::record_release`, which stops one
/// press being reported twice in the first place. This is the half that holds
/// even when a module reports a press nobody made.
#[tokio::test(start_paused = true)]
async fn a_press_that_arrives_before_the_light_is_not_a_hit() {
    use shared::proto::Command;

    /// Rounds that ended: a round finishes by switching its module back off.
    fn rounds_ended(sim: &SimHandle) -> usize {
        sim.history()
            .into_iter()
            .filter(|(_, command)| matches!(command, Command::SetOutput { on: false, .. }))
            .count()
    }

    /// Rounds that started.
    fn rounds_started(sim: &SimHandle) -> usize {
        sim.history()
            .into_iter()
            .filter(|(_, command)| matches!(command, Command::SetOutput { on: true, .. }))
            .count()
    }

    let (sim, task) = start(&["a"]);
    let only = ModuleId::from("a");

    eventually(|| sim.output(&only, 0) == Some(true), "the module to light").await;

    // One real press, ending one real round.
    sim.press(&only, 0);
    eventually(|| rounds_ended(&sim) == 1, "the round to end").await;

    // The duplicate, arriving while the game is between rounds. This is what a
    // bouncing switch used to send on release, ~100 ms behind the real press.
    sim.press(&only, 0);

    // Long enough for the next round to light (0.5-2 s) and, if the stale press
    // were credited, to end and for another to start.
    tokio::time::sleep(Duration::from_secs(4)).await;

    assert!(
        rounds_started(&sim) >= 2,
        "the next round never started, so this test proves nothing"
    );
    assert_eq!(
        rounds_ended(&sim),
        1,
        "a round ended with nobody pressing anything: the press that arrived          before the module lit was credited as a hit"
    );
    task.abort();
}

#[tokio::test(start_paused = true)]
async fn pressing_the_wrong_module_changes_nothing() {
    let (sim, task) = start(&["a", "b"]);

    eventually(|| sim.only_lit(0).is_some(), "a module to light").await;
    let lit = sim.only_lit(0).unwrap();
    let wrong = if lit == ModuleId::from("a") {
        ModuleId::from("b")
    } else {
        ModuleId::from("a")
    };

    for _ in 0..5 {
        sim.press(&wrong, 0);
    }
    tokio::time::sleep(Duration::from_millis(150)).await;

    assert_eq!(
        sim.only_lit(0),
        Some(lit),
        "a wrong press must not advance the round"
    );
    task.abort();
}

/// The scenario chose to tolerate this; the runtime only reports it. That
/// division is the point of `WaitError::ModuleLost` existing at all.
#[tokio::test(start_paused = true)]
async fn losing_an_unlit_module_does_not_end_the_round() {
    let (sim, task) = start(&["a", "b", "c"]);

    eventually(|| sim.only_lit(0).is_some(), "a module to light").await;
    let lit = sim.only_lit(0).unwrap();
    let other = ["a", "b", "c"]
        .iter()
        .map(|id| ModuleId::from(*id))
        .find(|id| *id != lit)
        .unwrap();

    sim.lose(&other);
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Still lit, still playable: pressing the lit one still advances.
    assert_eq!(sim.only_lit(0), Some(lit.clone()));
    sim.press(&lit, 0);
    eventually(
        || sim.output(&lit, 0) == Some(false),
        "the round to continue after an unrelated module was lost",
    )
    .await;

    assert!(!task.is_finished(), "the round should still be running");
    task.abort();
}

/// A round that ends must not leave the game stuck: the runtime re-acquires and
/// starts another one.
#[tokio::test(start_paused = true)]
async fn losing_the_lit_module_restarts_the_round() {
    let (sim, task) = start(&["a", "b"]);

    eventually(|| sim.only_lit(0).is_some(), "a module to light").await;
    let lit = sim.only_lit(0).unwrap();

    sim.lose(&lit);

    // Re-acquisition resets every output, then a new round lights one again.
    eventually(
        || sim.only_lit(0).is_some(),
        "a new round to start after the lit module was lost",
    )
    .await;
    assert!(!task.is_finished(), "the runtime should have re-acquired");
    task.abort();
}

#[tokio::test(start_paused = true)]
async fn every_module_starts_dark() {
    let (sim, task) = start(&["a", "b"]);
    // Before the first light, nothing should be on. Checked immediately, since
    // the scenario sleeps at least 500ms before lighting anything.
    assert_eq!(sim.output(&ModuleId::from("a"), 0), Some(false));
    assert_eq!(sim.output(&ModuleId::from("b"), 0), Some(false));
    task.abort();
}

/// Simon Says exists to test the *seam*, not the game.
///
/// It is structurally unlike whack-a-mole -- it waits on any module and checks
/// which one arrived, rather than waiting on a named one -- so if the runtime
/// API were shaped wrongly, writing this would have required changing it. It
/// did not.
mod simon {
    use super::*;
    use shared::proto::Command;

    /// The modules lit so far, in order, read from the command history.
    ///
    /// Reading history rather than polling LED state is what makes these tests
    /// deterministic: a blink is transient and whether a poll catches it depends
    /// on scheduling.
    fn lit_so_far(sim: &SimHandle) -> Vec<ModuleId> {
        sim.history()
            .into_iter()
            .filter_map(|(id, command)| match command {
                Command::SetOutput { on: true, .. } => Some(id),
                _ => None,
            })
            .collect()
    }

    /// Wait until the sequence shown so far is `want` long, and return it.
    async fn observe_sequence(sim: &SimHandle, want: usize) -> Vec<ModuleId> {
        eventually(
            || lit_so_far(sim).len() >= want,
            &format!("a sequence of {want} to be shown"),
        )
        .await;
        lit_so_far(sim)
    }

    /// The game-over blink is the only time two different modules are told to
    /// light with no `off` between them. Detecting it in the history means not
    /// having to catch it while it happens.
    fn game_over_blinked(sim: &SimHandle) -> bool {
        let mut lit: Option<ModuleId> = None;
        for (id, command) in sim.history() {
            match command {
                Command::SetOutput { on: true, .. } => {
                    if lit.as_ref().is_some_and(|other| *other != id) {
                        return true;
                    }
                    lit = Some(id);
                }
                _ => lit = None,
            }
        }
        false
    }

    #[tokio::test(start_paused = true)]
    async fn repeating_the_sequence_extends_it() {
        let (sim, task) = start_with(&["a", "b"], scenarios::simon_says);

        // Round one: a single flash.
        let first = observe_sequence(&sim, 1).await;
        assert_eq!(first.len(), 1, "first sequence should be one step");
        sim.press(&first[0], 0);

        // Getting it right means round two shows two flashes.
        let second = observe_sequence(&sim, 2).await;
        assert_eq!(second.len(), 2, "sequence should have grown to two steps");
        assert_eq!(
            second[0], first[0],
            "the sequence should be extended, not replaced"
        );

        task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn a_wrong_press_ends_the_round() {
        let (sim, task) = start_with(&["a", "b"], scenarios::simon_says);

        let shown = observe_sequence(&sim, 1).await;
        let wrong = if shown[0] == ModuleId::from("a") {
            ModuleId::from("b")
        } else {
            ModuleId::from("a")
        };
        assert!(!game_over_blinked(&sim), "the round should not be over yet");

        sim.press(&wrong, 0);

        eventually(
            || game_over_blinked(&sim),
            "the game-over blink after a wrong press",
        )
        .await;

        task.abort();
    }
}

/// The speedrun is the timing example: it alternates rather than randomising,
/// and counts wrong presses instead of ignoring them.
mod speedrun {
    use super::*;
    use shared::proto::Command;

    /// Modules told to light, in order.
    fn lit_order(sim: &SimHandle) -> Vec<ModuleId> {
        sim.history()
            .into_iter()
            .filter_map(|(id, command)| match command {
                Command::SetOutput { on: true, .. } => Some(id),
                _ => None,
            })
            .collect()
    }

    /// Press whatever is currently lit, `times` times, letting virtual time pass
    /// in between so the splits are not all zero.
    async fn run_laps(sim: &SimHandle, ids: &[&str], times: usize) {
        let ids: Vec<ModuleId> = ids.iter().map(|id| ModuleId(id.to_string())).collect();
        for _ in 0..times {
            eventually(
                || ids.iter().any(|id| sim.output(id, 0) == Some(true)),
                "a module to light",
            )
            .await;
            let lit = ids
                .iter()
                .find(|id| sim.output(id, 0) == Some(true))
                .unwrap()
                .clone();
            // Virtual time, so this costs nothing but gives a non-zero split.
            tokio::time::sleep(Duration::from_millis(250)).await;
            sim.press(&lit, 0);
            eventually(
                || sim.output(&lit, 0) == Some(false),
                "the lap to be recorded",
            )
            .await;
        }
    }

    #[tokio::test(start_paused = true)]
    async fn laps_alternate_between_modules() {
        let (sim, task) = start_with(&["a", "b"], scenarios::speedrun);
        run_laps(&sim, &["a", "b"], 4).await;

        let order = lit_order(&sim);
        assert!(order.len() >= 4, "expected at least 4 laps, got {order:?}");
        for pair in order[..4].windows(2) {
            assert_ne!(
                pair[0], pair[1],
                "a speedrun should alternate, not repeat: {order:?}"
            );
        }
        task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn a_wrong_press_does_not_advance_the_lap() {
        let (sim, task) = start_with(&["a", "b"], scenarios::speedrun);

        eventually(|| sim.only_lit(0).is_some(), "the first lap to start").await;
        let lit = sim.only_lit(0).unwrap();
        let wrong = if lit == ModuleId::from("a") {
            ModuleId::from("b")
        } else {
            ModuleId::from("a")
        };

        for _ in 0..3 {
            sim.press(&wrong, 0);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Still on the same lap: the same module is still lit.
        assert_eq!(sim.only_lit(0), Some(lit.clone()));
        assert_eq!(lit_order(&sim).len(), 1, "no extra lap should have started");

        task.abort();
    }

    /// The guard exists because a one-module speedrun has nothing to move
    /// between, and would silently light the same board forever.
    #[tokio::test(start_paused = true)]
    async fn one_module_is_refused_with_a_reason() {
        let (_sim, task) = start_with(&["a"], scenarios::speedrun);
        // The runtime catches the error, logs it and re-acquires, so the check
        // is that no lap ever starts rather than that the task ends.
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert!(!task.is_finished());
        task.abort();

        // Assert the message itself is useful, by calling it directly.
        let (mut link, _handle) = SimLink::new(vec![(ModuleId::from("a"), sim::button(1, 1))]);
        let err = runtime::run_once(&mut link, scenarios::speedrun)
            .await
            .unwrap_err();
        let rendered = format!("{err:#}");
        assert!(rendered.contains("at least 2 modules"), "{rendered}");
    }
}

/// Tests for the coin scenario.
///
/// These assert on *command history* rather than on output state wherever
/// money is involved. A credit is consumed by a round that finishes, and by
/// then any LED it lit is already off again -- polling for it is exactly the
/// flakiness the history was added to avoid.
mod arcade {
    use super::*;
    use shared::proto::Command;

    /// A bench with a coin slot and two buttons, matching `Bench::for_scenario`.
    fn start_arcade() -> (SimHandle, tokio::task::JoinHandle<anyhow::Result<()>>) {
        let specs = vec![
            (ModuleId::from("slot"), sim::coin()),
            (ModuleId::from("a"), sim::button(1, 1)),
            (ModuleId::from("b"), sim::button(1, 1)),
        ];
        let (link, handle) = SimLink::new(specs);
        let task = tokio::spawn(runtime::run(link.quiet(), scenarios::arcade));
        (handle, task)
    }

    /// How many credits have been played to completion.
    ///
    /// A round ends by switching its button back off, so counting those is a
    /// durable measure of progress -- unlike polling for the lit button, which
    /// the next credit may re-light before the test looks.
    fn rounds_finished(sim: &SimHandle) -> usize {
        sim.history()
            .into_iter()
            .filter(|(_, command)| {
                matches!(
                    command,
                    Command::SetOutput {
                        channel: 0,
                        on: false
                    }
                )
            })
            .count()
    }

    /// Whether `arcade` has reset the modules, which it does once, right before
    /// it starts waiting for a coin.
    fn in_attract_mode(sim: &SimHandle) -> bool {
        sim.history()
            .iter()
            .any(|(id, command)| id == &ModuleId::from("slot") && *command == Command::Reset)
    }

    #[tokio::test(start_paused = true)]
    async fn nothing_lights_before_anyone_has_paid() {
        let (sim, task) = start_arcade();
        eventually(|| in_attract_mode(&sim), "attract mode").await;
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert_eq!(sim.only_lit(0), None, "a button lit before anyone paid");
        task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn a_coin_buys_a_round() {
        let (sim, task) = start_arcade();
        eventually(|| in_attract_mode(&sim), "attract mode").await;

        sim.insert_coin(&ModuleId::from("slot"), 1);

        eventually(|| sim.only_lit(0).is_some(), "a button to light up").await;
        task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn a_three_pulse_coin_buys_three_rounds() {
        let (sim, task) = start_arcade();
        eventually(|| in_attract_mode(&sim), "attract mode").await;

        sim.insert_coin(&ModuleId::from("slot"), 3);

        // Play every credit through, pressing whatever lights up.
        //
        // Progress is measured from the history, not from watching the lit
        // button go dark: the next credit picks a button at random and may
        // re-light the same one immediately, so the dark moment is transient and
        // a test that polls for it races.
        for credit in 0..3 {
            eventually(
                || sim.only_lit(0).is_some(),
                &format!("credit {} to light a button", credit + 1),
            )
            .await;
            sim.press(&sim.only_lit(0).unwrap(), 0);
            eventually(
                || rounds_finished(&sim) > credit,
                &format!("credit {} to be spent", credit + 1),
            )
            .await;
        }
        task.abort();
    }

    /// Integer division means a coin under the price buys nothing. Worth a test
    /// because the alternative -- rounding up -- is a free game.
    #[tokio::test(start_paused = true)]
    async fn a_coin_worth_no_credits_does_not_start_a_game() {
        let (sim, task) = start_arcade();
        eventually(|| in_attract_mode(&sim), "attract mode").await;

        sim.insert_coin(&ModuleId::from("slot"), 0);

        // Give the scenario room to do the wrong thing before concluding it did
        // not.
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert_eq!(sim.only_lit(0), None, "a 0-pulse coin started a game");
        task.abort();
    }

    /// The check that turns a wiring mistake into a message instead of a hang.
    #[tokio::test(start_paused = true)]
    async fn arcade_refuses_a_bench_with_no_coin_acceptor() {
        let specs = vec![
            (ModuleId::from("a"), sim::button(1, 1)),
            (ModuleId::from("b"), sim::button(1, 1)),
        ];
        let (mut link, _handle) = SimLink::new(specs);
        let err = runtime::run_once(&mut link, scenarios::arcade)
            .await
            .expect_err("arcade without a coin slot must fail, not wait forever");
        assert!(
            format!("{err:#}").contains("coin acceptor"),
            "should say what is missing: {err:#}"
        );
    }

    /// The mirror of the test above: money in, but nothing to play.
    #[tokio::test(start_paused = true)]
    async fn arcade_refuses_a_bench_with_nothing_to_play() {
        let specs = vec![(ModuleId::from("slot"), sim::coin())];
        let (mut link, _handle) = SimLink::new(specs);
        let err = runtime::run_once(&mut link, scenarios::arcade)
            .await
            .expect_err("a coin slot with no buttons must fail, not take money");
        assert!(
            format!("{err:#}").contains("button"),
            "should say what is missing: {err:#}"
        );
    }

    /// The descriptors have to actually arrive, or `require_role` is a no-op and
    /// `arcade` cannot tell a coin slot from a button. This is the regression
    /// test for `Modules::descriptors` having been left permanently empty.
    #[tokio::test(start_paused = true)]
    async fn every_module_reports_what_it_is_before_the_scenario_starts() {
        let specs = vec![
            (ModuleId::from("slot"), sim::coin()),
            (ModuleId::from("a"), sim::button(1, 1)),
        ];
        let (mut link, _handle) = SimLink::new(specs);
        let _ = runtime::run_once(
            &mut link,
            |modules: std::sync::Arc<runtime::Modules>| async move {
                let slot = modules
                    .descriptor(&ModuleId::from("slot"))
                    .expect("the coin slot never introduced itself");
                assert_eq!(slot.role, shared::proto::Role::Coin);
                let button = modules
                    .descriptor(&ModuleId::from("a"))
                    .expect("the button never introduced itself");
                assert_eq!(button.role, shared::proto::Role::Button);
                Ok(())
            },
        )
        .await;
    }
}
