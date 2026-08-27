//! Scenario tests, run against the simulated link.
//!
//! These are the first tests in this project that exercise *game logic* rather
//! than parsing or arithmetic. Everything they touch -- the runtime, the
//! scenario, the protocol messages -- is the same code the BLE path runs; only
//! the transport is swapped.

use std::time::Duration;

use crate::link::ModuleId;
use crate::link::sim::{SimHandle, SimLink};
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
    let ids: Vec<ModuleId> = ids.iter().map(|id| ModuleId(id.to_string())).collect();
    let (link, handle) = SimLink::new(ids, 1, 1);
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
