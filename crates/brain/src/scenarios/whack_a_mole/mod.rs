use std::sync::Arc;
use std::time::Duration;

use log::{debug, info, warn};
use rand::rngs::SmallRng;
use rand::{RngCore, SeedableRng};

use crate::runtime::{Modules, WaitError, require};

/// How long a player gets before the round gives up on them.
const PATIENCE: Duration = Duration::from_secs(30);

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

        // A press that arrived before this module lit is not a hit on it. It is
        // the tail of the last round -- a second press, a duplicate from a
        // bouncing switch -- or someone leaning on a button between rounds.
        // Left queued, it is spent the instant the round starts: the box lights
        // and goes dark again with nobody touching it.
        let stale = modules.drop_pending_presses().await;
        if stale > 0 {
            debug!("dropped {stale} press(es) that arrived before {target} lit");
        }

        info!("module {target} is lit");

        // Wait for the right one, tolerating wrong presses.
        //
        // Each way out of this loop reports itself, rather than the round
        // announcing a hit afterwards: a timeout also ends the round, and a
        // shared "hit" line after the loop claimed a press that never happened.
        // The log is the only view into a running game, so it must not invent
        // events.
        loop {
            match modules.wait_for_press(&target, PATIENCE).await {
                Ok(_channel) => {
                    info!("module {target} hit");
                    break;
                }
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

        modules.set_output(&target, 0, false).await?;
    }
}

#[cfg(test)]
mod tests;
