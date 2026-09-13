use std::sync::Arc;
use std::time::Duration;

use log::{info, warn};

use crate::runtime::{Modules, WaitError, require};

/// How long a player gets before the run gives up on them.
const PATIENCE: Duration = Duration::from_secs(30);

/// How many presses make one speedrun.
const SPEEDRUN_LAPS: usize = 10;

/// Render a duration the way a stopwatch would.
fn stopwatch(d: Duration) -> String {
    format!("{:.3}s", d.as_secs_f64())
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

#[cfg(test)]
mod tests;
