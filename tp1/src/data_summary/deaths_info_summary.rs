use crate::data_summary::player_stats_summary::PlayerStatsSummary;
use crate::data_summary::weapon_stats_summary::WeaponStatsSummary;

use serde::Serialize;
use std::collections::HashMap;

/// A summary of deaths information, including top killers and top weapons.
///
/// This struct aggregates and serializes stats for the players with the most deaths
/// and the weapons with the most deaths.
///
/// # Fields
///
/// * `top_killers` - A `HashMap` where:
///   - The key is the player's name (a `String`).
///   - The value is a `PlayerStatsSummary` that contains aggregated statistics for the player.
///
/// * `top_weapons` - A `HashMap` where:
///   - The key is the weapon's name (a `String`).
///   - The value is a `WeaponStatsSummary` that contains aggregated statistics for the weapon.
#[derive(Serialize, Debug)]
pub struct DeathsInfoSummary {
    pub top_killers: HashMap<String, PlayerStatsSummary>,
    pub top_weapons: HashMap<String, WeaponStatsSummary>,
}
