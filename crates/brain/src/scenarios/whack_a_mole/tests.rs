use std::time::Duration;

use super::*;
use crate::link::ModuleId;
use crate::link::sim::SimHandle;
use crate::scenarios::testing::{eventually, start_with};

fn start(ids: &[&str]) -> (SimHandle, tokio::task::JoinHandle<anyhow::Result<()>>) {
    start_with(ids, whack_a_mole)
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
/// Counts rounds from the history rather than polling for the light: a
/// phantom round opens and closes between two polls, so watching the LED
/// can miss it entirely.
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
