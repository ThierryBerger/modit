//! Simon Says exists to test the *seam*, not the game.
//!
//! It is structurally unlike whack-a-mole -- it waits on any module and checks
//! which one arrived, rather than waiting on a named one -- so if the runtime
//! API were shaped wrongly, writing this would have required changing it. It
//! did not.

use super::*;
use crate::link::ModuleId;
use crate::link::sim::SimHandle;
use crate::scenarios::testing::{eventually, start_with};
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
    let (sim, task) = start_with(&["a", "b"], simon_says);

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
    let (sim, task) = start_with(&["a", "b"], simon_says);

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
