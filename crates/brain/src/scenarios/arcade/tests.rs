//! Tests for the coin scenario.
//!
//! These assert on *command history* rather than on output state wherever
//! money is involved. A credit is consumed by a round that finishes, and by
//! then any LED it lit is already off again -- polling for it is exactly the
//! flakiness the history was added to avoid.

use super::*;
use crate::link::ModuleId;
use crate::link::sim::{self, SimHandle, SimLink};
use crate::runtime;
use crate::scenarios::testing::eventually;
use shared::proto::Command;

/// A bench with a coin slot and two buttons, matching `Bench::for_scenario`.
fn start_arcade() -> (SimHandle, tokio::task::JoinHandle<anyhow::Result<()>>) {
    let specs = vec![
        (ModuleId::from("slot"), sim::coin()),
        (ModuleId::from("a"), sim::button(1, 1)),
        (ModuleId::from("b"), sim::button(1, 1)),
    ];
    let (link, handle) = SimLink::new(specs);
    let task = tokio::spawn(runtime::run(link.quiet(), arcade));
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
    let err = runtime::run_once(&mut link, arcade)
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
    let err = runtime::run_once(&mut link, arcade)
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
