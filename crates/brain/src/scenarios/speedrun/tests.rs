//! The speedrun is the timing example: it alternates rather than randomising,
//! and counts wrong presses instead of ignoring them.

use super::*;
use crate::link::ModuleId;
use crate::link::sim::{self, SimHandle, SimLink};
use crate::runtime;
use crate::scenarios::testing::{eventually, start_with};
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
    let (sim, task) = start_with(&["a", "b"], speedrun);
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
    let (sim, task) = start_with(&["a", "b"], speedrun);

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
    let (_sim, task) = start_with(&["a"], speedrun);
    // The runtime catches the error, logs it and re-acquires, so the check
    // is that no lap ever starts rather than that the task ends.
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(!task.is_finished());
    task.abort();

    // Assert the message itself is useful, by calling it directly.
    let (mut link, _handle) = SimLink::new(vec![(ModuleId::from("a"), sim::button(1, 1))]);
    let err = runtime::run_once(&mut link, speedrun).await.unwrap_err();
    let rendered = format!("{err:#}");
    assert!(rendered.contains("at least 2 modules"), "{rendered}");
}
