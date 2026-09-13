use std::sync::Arc;
use std::time::Duration;

use log::{info, warn};
use rand::rngs::SmallRng;
use rand::{RngCore, SeedableRng};

use shared::proto::Role;

use crate::runtime::{Modules, WaitError, require_role};

/// How long a player gets to use a credit before it is forfeited.
const PATIENCE: Duration = Duration::from_secs(30);

/// How many pulses the acceptor emits per credit.
///
/// **Venue policy, not hardware.** A CH-92x is taught which coins it takes and
/// how many pulses each one is worth, so this number is a statement about how
/// *your* acceptor was programmed and what you charge. It lives here, in the
/// part of the project you are meant to edit, rather than in the firmware you
/// flash once -- see [`Event::Coin`](shared::proto::Event::Coin).
const PULSES_PER_CREDIT: u8 = 1;

/// How long attract mode waits for someone to pay.
const ATTRACT_PATIENCE: Duration = Duration::from_secs(600);

/// Pay to play: a coin buys a round of whack-a-mole.
///
/// The first scenario to use more than one *kind* of module, and the reason
/// [`Descriptor::role`](shared::proto::Descriptor::role) exists -- it sorts the
/// modules into the coin slot and the buttons rather than the scenario
/// hardcoding which id is which.
pub async fn arcade(modules: Arc<Modules>) -> anyhow::Result<()> {
    // Sort the modules by what they said they were.
    let mut slot = None;
    let mut buttons = Vec::new();
    for id in modules.ids() {
        match modules.descriptor(id).map(|d| d.role) {
            Some(Role::Coin) => slot = Some(id.clone()),
            Some(Role::Button) => buttons.push(id.clone()),
            // No descriptor means a transport that does not carry them. Refuse
            // rather than guess: picking the wrong module as the coin slot
            // produces a game that never starts and never says why.
            None => anyhow::bail!(
                "module {id} never said what it was, so arcade cannot tell the \
                 coin slot from the buttons"
            ),
        }
    }

    let Some(slot) = slot else {
        anyhow::bail!("arcade needs a coin acceptor; none of the modules is one");
    };
    if buttons.is_empty() {
        anyhow::bail!("arcade needs at least one button module to play with");
    }

    require_role(&modules, &slot, Some(Role::Coin), 1, 0)?;
    for id in &buttons {
        require_role(&modules, id, Some(Role::Button), 1, 1)?;
    }

    let mut rng = SmallRng::from_os_rng();
    modules.reset_all().await?;

    loop {
        // Attract mode: waiting to be paid.
        info!("insert coin ({} button(s) ready)", buttons.len());

        let pulses = match modules.next_coin(ATTRACT_PATIENCE).await {
            Ok((from, pulses)) if from == slot => pulses,
            Ok((from, pulses)) => {
                warn!("ignoring {pulses} pulse(s) from {from}, which is not the coin slot");
                continue;
            }
            Err(WaitError::Timeout) => {
                info!("nobody played for {ATTRACT_PATIENCE:?}");
                return Ok(());
            }
            Err(err) => return Err(err.into()),
        };

        // Integer division on purpose: a coin worth less than a credit buys
        // nothing and is not refundable, which is what a real cabinet does.
        let credits = pulses / PULSES_PER_CREDIT;
        if credits == 0 {
            warn!("{pulses} pulse(s) is under the {PULSES_PER_CREDIT}-pulse price; no credit");
            continue;
        }
        info!("{pulses} pulse(s) -> {credits} credit(s)");

        for credit in 1..=credits {
            info!("credit {credit}/{credits}");
            let target = buttons[(rng.next_u64() % buttons.len() as u64) as usize].clone();
            modules.set_output(&target, 0, true).await?;

            match modules.wait_for_press(&target, PATIENCE).await {
                Ok(_) => info!("hit"),
                Err(WaitError::Timeout) => warn!("no press within {PATIENCE:?}; credit forfeited"),
                // A lost module mid-game means a credit that was paid for
                // cannot be delivered. Stop rather than pretend.
                Err(err) => {
                    modules.set_output(&target, 0, false).await?;
                    return Err(err.into());
                }
            }
            modules.set_output(&target, 0, false).await?;
        }

        info!("game over, {credits} credit(s) played");
    }
}

#[cfg(test)]
mod tests;
