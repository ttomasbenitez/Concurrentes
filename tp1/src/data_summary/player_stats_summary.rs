use serde::Serialize;
use std::collections::HashMap;

/// Summary of a player's statistics.
///
/// # Fields
///
/// * `deaths` - The total number of deaths for the player.
/// * `weapons_percentage` - A `HashMap` where keys are weapon names and values are the percentage of total deaths caused by each weapon.
#[derive(Serialize, Debug)]
pub struct PlayerStatsSummary {
    pub deaths: u32,
    pub weapons_percentage: HashMap<String, f64>,
}
