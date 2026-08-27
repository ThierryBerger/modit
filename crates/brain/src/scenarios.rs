//! Scenarios: the part you write.
//!
//! A scenario is an `async fn` over [`Modules`]. It never mentions BLE, a
//! peripheral, a characteristic or a task -- which is the whole point of the
//! seam. The same code runs against real boards and against `--simulate`.
//!
//! Three worked examples, deliberately different from each other:
//!
//! | Scenario | Waits with | Reacts to a wrong press | Ends |
//! | -------- | ---------- | ----------------------- | ---- |
//! | [`whack_a_mole`] | [`Modules::wait_for_press`] on one module | ignores it | never |
//! | [`simon_says`] | [`Modules::next_press`] on any module | ends the round | on a mistake |
//! | [`speedrun`] | [`Modules::next_press`] on any module | counts it as a miss | after a fixed number of laps |
//!
//! If you are writing a fourth, start by copying whichever of these waits the
//! way yours needs to.

use std::sync::Arc;
use std::time::Duration;

use log::{info, warn};
use rand::rngs::SmallRng;
use rand::{RngCore, SeedableRng};

use crate::runtime::{Modules, WaitError, require};

/// How long a player gets before the round gives up on them.
const PATIENCE: Duration = Duration::from_secs(30);

/// How many presses make one speedrun.
const SPEEDRUN_LAPS: usize = 10;

/// Render a duration the way a stopwatch would.
fn stopwatch(d: Duration) -> String {
    format!("{:.3}s", d.as_secs_f64())
}

/// Light one module at a time; the player presses whichever is lit.
pub async fn whack_a_mole(modules: Arc<Modules>) -> anyhow::Result<()> {
    // Fail before the game starts if a board cannot do what this needs.
    for id in modules.ids() {
        require(&modules, id, 1, 1)?;
    }

    // Seeded from the OS, so runs differ.
    let mut rng = SmallRng::from_os_rng();
    let ids = modules.ids().to_vec();

    modules.reset_all().await?;

    loop {
        // Pause, then light one at random.
        let delay = Duration::from_millis(rng.next_u64() % 1500 + 500);
        tokio::time::sleep(delay).await;

        let target = ids[(rng.next_u64() % ids.len() as u64) as usize].clone();
        modules.set_output(&target, 0, true).await?;
        info!("module {target} is lit");

        // Wait for the right one, tolerating wrong presses.
        loop {
            match modules.wait_for_press(&target, PATIENCE).await {
                Ok(_channel) => break,
                Err(WaitError::Timeout) => {
                    warn!("nobody pressed {target} within {PATIENCE:?}; moving on");
                    break;
                }
                // Whack-a-mole can carry on without a module, unless it was the
                // lit one. Simon Says would end the round here instead -- which
                // is why the runtime reports this rather than deciding.
                Err(WaitError::ModuleLost(lost)) if lost != target => {
                    warn!("module {lost} dropped out; continuing without it");
                }
                Err(err) => return Err(err.into()),
            }
        }

        info!("module {target} hit");
        modules.set_output(&target, 0, false).await?;
    }
}

/// Show a growing sequence, then have the player repeat it in order.
///
/// Written as the test of whether the seam is in the right place: it is
/// structurally unlike whack-a-mole. It needs "the next press, whichever
/// module" rather than "a press on this module", and a wrong press must end the
/// round rather than being ignored.
pub async fn simon_says(modules: Arc<Modules>) -> anyhow::Result<()> {
    for id in modules.ids() {
        require(&modules, id, 1, 1)?;
    }

    let mut rng = SmallRng::from_os_rng();
    let ids = modules.ids().to_vec();
    let mut sequence = Vec::new();

    modules.reset_all().await?;

    loop {
        // Extend the sequence by one, then play the whole thing back.
        sequence.push(ids[(rng.next_u64() % ids.len() as u64) as usize].clone());
        info!("showing a sequence of {}", sequence.len());

        for id in &sequence {
            modules.set_output(id, 0, true).await?;
            tokio::time::sleep(Duration::from_millis(400)).await;
            modules.set_output(id, 0, false).await?;
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        // Now the player repeats it. This waits on *any* module and checks which
        // one arrived -- the reason the runtime's primitive is "next press from
        // anywhere" rather than "a press on this module".
        for (step, expected) in sequence.iter().enumerate() {
            match modules.next_press(PATIENCE).await {
                Ok((pressed, _)) if &pressed == expected => {
                    info!("step {} correct", step + 1);
                }
                Ok((pressed, _)) => {
                    warn!("step {} wrong: {pressed} instead of {expected}", step + 1);
                    return round_over(&modules, sequence.len() - 1).await;
                }
                Err(WaitError::Timeout) => {
                    warn!("nobody answered within {PATIENCE:?}");
                    return round_over(&modules, sequence.len() - 1).await;
                }
                // Unlike whack-a-mole, any loss ends this round: the sequence
                // refers to specific modules and cannot continue without them.
                Err(err) => return Err(err.into()),
            }
        }
    }
}

/// Blink everything, announce the score, and end the round.
async fn round_over(modules: &Modules, score: usize) -> anyhow::Result<()> {
    info!("round over, score {score}");
    for _ in 0..3 {
        for id in modules.ids() {
            modules.set_output(id, 0, true).await?;
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
        for id in modules.ids() {
            modules.set_output(id, 0, false).await?;
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    Ok(())
}

/// A timed run: two or more modules light in turn, and every split is reported.
///
/// The point is the clock, not the puzzle -- press as fast as you can, see where
/// the time went. Uses [`Modules::next_press`] rather than
/// [`Modules::wait_for_press`] so that pressing the wrong module can be counted
/// as a miss instead of silently ignored.
pub async fn speedrun(modules: Arc<Modules>) -> anyhow::Result<()> {
    for id in modules.ids() {
        require(&modules, id, 1, 1)?;
    }
    let ids = modules.ids().to_vec();
    if ids.len() < 2 {
        anyhow::bail!(
            "speedrun needs at least 2 modules, found {}: there is nothing to \
             move between",
            ids.len()
        );
    }

    modules.reset_all().await?;
    info!(
        "speedrun: {SPEEDRUN_LAPS} laps across {} modules",
        ids.len()
    );
    info!("get ready...");
    tokio::time::sleep(Duration::from_secs(2)).await;

    let mut splits: Vec<Duration> = Vec::with_capacity(SPEEDRUN_LAPS);
    let mut misses = 0usize;
    let started = tokio::time::Instant::now();

    for lap in 0..SPEEDRUN_LAPS {
        // Alternate around the modules, so the player always knows where to
        // look next -- unlike whack-a-mole, this is a race, not a surprise.
        let target = &ids[lap % ids.len()];
        modules.set_output(target, 0, true).await?;
        let lit_at = tokio::time::Instant::now();

        // Keep waiting until the *right* module is pressed, counting the rest.
        let split = loop {
            match modules.next_press(PATIENCE).await {
                Ok((pressed, _)) if &pressed == target => break lit_at.elapsed(),
                Ok((pressed, _)) => {
                    misses += 1;
                    warn!("miss: pressed {pressed}, wanted {target}");
                }
                Err(WaitError::Timeout) => {
                    warn!("no press within {PATIENCE:?}; abandoning the run");
                    modules.set_output(target, 0, false).await?;
                    return Ok(());
                }
                // A lost module mid-run invalidates the times, so stop.
                Err(err) => return Err(err.into()),
            }
        };

        modules.set_output(target, 0, false).await?;
        splits.push(split);
        info!(
            "lap {:>2}/{SPEEDRUN_LAPS}  {}  ({} total)",
            lap + 1,
            stopwatch(split),
            stopwatch(started.elapsed())
        );
    }

    report(&splits, misses);

    // A short celebration, then the runtime starts another run.
    for _ in 0..3 {
        for id in &ids {
            modules.set_output(id, 0, true).await?;
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
        for id in &ids {
            modules.set_output(id, 0, false).await?;
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
    tokio::time::sleep(Duration::from_secs(3)).await;
    Ok(())
}

/// Print the scoreboard for a finished run.
fn report(splits: &[Duration], misses: usize) {
    let total: Duration = splits.iter().sum();
    let best = splits.iter().min().copied().unwrap_or_default();
    let worst = splits.iter().max().copied().unwrap_or_default();
    let average = total / splits.len().max(1) as u32;

    info!("---- run complete ----");
    info!("  total    {}", stopwatch(total));
    info!("  best     {}", stopwatch(best));
    info!("  average  {}", stopwatch(average));
    info!("  worst    {}", stopwatch(worst));
    if misses > 0 {
        info!("  misses   {misses}");
    }
}
