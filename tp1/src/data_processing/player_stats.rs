use crate::data_processing::weapon_stats::WeaponStats;
use std::collections::HashMap;

/// Represents statistics for a player.
///
/// # Fields
///
/// * `used_weapons` - A `HashMap` where keys are weapon names and values are their respective `WeaponStats`.
/// * `deaths` - The total number of deaths recorded for the player.
#[derive(Debug)]
pub struct PlayerStats {
    pub(crate) used_weapons: HashMap<String, WeaponStats>,
    pub(crate) deaths: u32,
}
