/// Represents statistics for a weapon.
///
/// # Fields
///
/// * `deaths` - The number of deaths for the weapon.
/// * `valid_distances_count` - The number of valid distance measurements recorded for this weapon.
///     That is, when all position fields for killer and victim have a valid f64 value.
/// * `total_distance` - The sum of all distances of all valid measurements involving this weapon.
#[derive(Debug)]
pub struct WeaponStats {
    pub(crate) deaths: u32,
    pub(crate) valid_distances_count: u32,
    pub(crate) total_distance: f64,
}
