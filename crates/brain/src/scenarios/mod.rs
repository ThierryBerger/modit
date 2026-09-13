//! Scenarios: the part you write.
//!
//! A scenario is an `async fn` over [`Modules`](crate::runtime::Modules). It
//! never mentions BLE, a peripheral, a characteristic or a task -- which is the
//! whole point of the seam. The same code runs against real boards and against
//! `--simulate`.
//!
//! Each scenario is a folder: `mod.rs` holds its rules and tuning constants,
//! `tests.rs` its tests. Worked examples, deliberately different from each
//! other:
//!
//! | Scenario | Waits with | Reacts to a wrong press | Ends |
//! | -------- | ---------- | ----------------------- | ---- |
//! | [`whack_a_mole`](whack_a_mole()) | [`Modules::wait_for_press`](crate::runtime::Modules::wait_for_press) on one module | ignores it | never |
//! | [`simon_says`](simon_says()) | [`Modules::next_press`](crate::runtime::Modules::next_press) on any module | ends the round | on a mistake |
//! | [`speedrun`](speedrun()) | [`Modules::next_press`](crate::runtime::Modules::next_press) on any module | counts it as a miss | after a fixed number of laps |
//! | [`arcade`](arcade()) | [`Modules::next_coin`](crate::runtime::Modules::next_coin), then per-module presses | forfeits the credit | when nobody pays |
//!
//! If you are writing another, copy whichever of these folders waits the way
//! yours needs to, declare it below, and add it to the `match` in `main.rs`.

mod arcade;
mod simon_says;
mod speedrun;
mod whack_a_mole;

#[cfg(test)]
mod testing;

pub use arcade::arcade;
pub use simon_says::simon_says;
pub use speedrun::speedrun;
pub use whack_a_mole::whack_a_mole;
