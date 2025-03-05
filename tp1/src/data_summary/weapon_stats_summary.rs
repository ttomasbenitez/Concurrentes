use serde::Serialize;

/// Summary of weapon statistics.
///
/// # Fields
///
/// * `deaths_percentage` - The percentage of total deaths caused by this weapon.
/// * `average_distance` - The average distance of deaths caused by this weapon.
#[derive(Serialize, Debug)]
pub struct WeaponStatsSummary {
    pub deaths_percentage: f64,
    pub average_distance: f64,
}
