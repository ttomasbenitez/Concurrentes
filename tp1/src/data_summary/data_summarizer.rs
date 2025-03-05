use crate::data_processing::player_stats::PlayerStats;
use crate::data_processing::weapon_stats::WeaponStats;
use crate::data_summary::deaths_info_summary::DeathsInfoSummary;
use crate::data_summary::player_stats_summary::PlayerStatsSummary;
use crate::data_summary::weapon_stats_summary::WeaponStatsSummary;

use std::collections::HashMap;

/// Aggregates player and weapon statistics from the given deaths information.
///
/// This function processes the deaths information to compute aggregated statistics, including top killers and top weapons.
/// It calculates weapon statistics, total deaths, and then determines the top killers and top weapons based on these statistics.
///
/// # Arguments
///
/// * `deaths_info` - A `HashMap` where keys are player names and values are their respective `PlayerStats`.
///
/// # Returns
///
/// A `DeathsInfoSummary` containing:
/// - `top_killers`: A `HashMap` of the top 10 players by kills and their statistics.
/// - `top_weapons`: A `HashMap` of the top 10 weapons by kills and their statistics.
pub fn summarize(deaths_info: HashMap<String, PlayerStats>) -> DeathsInfoSummary {
    let weapon_stats = compute_weapon_stats(&deaths_info);
    let total_deaths = calculate_total_deaths(&weapon_stats);

    let top_killers = process_top_killers(deaths_info);
    let top_weapons = process_top_weapons(weapon_stats, total_deaths);

    DeathsInfoSummary {
        top_killers,
        top_weapons,
    }
}

/// Computes aggregated statistics for weapons from the given player deaths information.
///
/// # Arguments
///
/// * `deaths_info` - A `HashMap` where keys are player names and values are their respective `PlayerStats`.
///
/// # Returns
///
/// A `HashMap` where keys are weapon names and values are the total `WeaponStats` for that weapon.
fn compute_weapon_stats(
    deaths_info: &HashMap<String, PlayerStats>,
) -> HashMap<String, WeaponStats> {
    let mut weapon_stats: HashMap<String, WeaponStats> = HashMap::new();
    for player in deaths_info.values() {
        for (weapon, stats) in &player.used_weapons {
            let entry = weapon_stats.entry(weapon.clone()).or_insert(WeaponStats {
                deaths: 0,
                valid_distances_count: 0,
                total_distance: 0.0,
            });
            entry.deaths += stats.deaths;
            entry.valid_distances_count += stats.valid_distances_count;
            entry.total_distance += stats.total_distance;
        }
    }
    weapon_stats
}

/// Calculates the total number of deaths from the weapon statistics.
///
/// # Arguments
///
/// * `weapon_stats` - A `HashMap` where keys are weapon names and values are `WeaponStats`.
///
/// # Returns
///
/// The total number of deaths across all weapons.
fn calculate_total_deaths(weapon_stats: &HashMap<String, WeaponStats>) -> u32 {
    weapon_stats.values().map(|stats| stats.deaths).sum()
}

/// Processes the top killers from the given player deaths information.
///
/// This function identifies the top 10 players based on the number of deaths and computes:
/// - The percentage of total deaths made with each weapon for these top players.
///
/// # Arguments
///
/// * `deaths_info` - A `HashMap` where keys are player names and values are their respective `PlayerStats`.
///
/// # Returns
///
/// A `HashMap` where keys are player names and values are `PlayerStatsSummary` for the top 10 players by deaths.
fn process_top_killers(
    deaths_info: HashMap<String, PlayerStats>,
) -> HashMap<String, PlayerStatsSummary> {
    let player_vec = sort_players_by_kills(deaths_info);

    let mut top_killers = HashMap::new();
    for (player_name, stats) in player_vec.into_iter().take(10) {
        let weapon_percentage =
            calculate_player_weapon_percentage(stats.used_weapons, stats.deaths);
        top_killers.insert(
            player_name,
            PlayerStatsSummary {
                deaths: stats.deaths,
                weapons_percentage: weapon_percentage,
            },
        );
    }

    top_killers
}

/// Sorts players with valid names by the number of kills in descending order.
///
/// # Arguments
///
/// * `deaths_info` - A `HashMap` where keys are player names and values are their respective `PlayerStats`.
///
/// # Returns
///
/// A `Vec` of tuples where each tuple contains a player name and their `PlayerStats`, sorted by the number of deaths.
fn sort_players_by_kills(deaths_info: HashMap<String, PlayerStats>) -> Vec<(String, PlayerStats)> {
    let mut player_vec: Vec<_> = deaths_info
        .into_iter()
        .filter(|(killer_name, _)| !killer_name.is_empty())
        .collect();

    player_vec.sort_by(|p1, p2| p2.1.deaths.cmp(&p1.1.deaths).then_with(|| p1.0.cmp(&p2.0)));

    player_vec
}

/// Calculates the percentage of total deaths for each weapon used by a player.
///
/// # Arguments
///
/// * `used_weapons` - A `HashMap` where keys are weapon names and values are `WeaponStats` for each weapon.
/// * `total_kills` - The total number of kills made by the player.
///
/// # Returns
///
/// A `HashMap` where keys are weapon names and values are the percentage of total kills made with each weapon.
fn calculate_player_weapon_percentage(
    used_weapons: HashMap<String, WeaponStats>,
    total_kills: u32,
) -> HashMap<String, f64> {
    if total_kills == 0 {
        return HashMap::new();
    }

    let weapon_vec = sort_weapons_by_kills(used_weapons);

    let mut weapon_percentage = HashMap::new();
    let top_weapons = weapon_vec.into_iter().take(3).collect::<Vec<_>>();
    for (weapon, stats) in top_weapons {
        let percentage = (stats.deaths as f64 / total_kills as f64 * 10000.0).round() / 100.0;
        weapon_percentage.insert(weapon, percentage);
    }

    weapon_percentage
}

/// Processes the top weapons by deaths from the given weapon statistics.
///
/// This function identifies the top 10 weapons based on the number of deaths and computes:
/// - The percentage of total deaths made with each weapon.
/// - The average distance for deaths made with each weapon.
///
/// # Arguments
///
/// * `weapon_stats` - A `HashMap` where keys are weapon names and values are `WeaponStats`.
/// * `total_deaths` - The total number of deaths across all weapons.
///
/// # Returns
///
/// A `HashMap` where keys are weapon names and values are `WeaponStatsSummary` for the top 10 weapons.
fn process_top_weapons(
    weapon_stats: HashMap<String, WeaponStats>,
    total_deaths: u32,
) -> HashMap<String, WeaponStatsSummary> {
    let sorted_weapon_vec = sort_weapons_by_kills(weapon_stats);

    let mut top_weapons = HashMap::new();
    for (weapon, stats) in sorted_weapon_vec.into_iter().take(10) {
        if total_deaths == 0 {
            continue;
        }

        let deaths_percentage =
            (stats.deaths as f64 / total_deaths as f64 * 10000.0).round() / 100.0;
        let avg_distance =
            calculate_average_distance(stats.valid_distances_count, stats.total_distance);

        top_weapons.insert(
            weapon,
            WeaponStatsSummary {
                deaths_percentage,
                average_distance: avg_distance,
            },
        );
    }

    top_weapons
}

/// Sorts weapons by the number of deaths in descending order.
///
/// # Arguments
///
/// * `used_weapons` - A `HashMap` where keys are weapon names and values are `WeaponStats` for each weapon.
///
/// # Returns
///
/// A `Vec` of tuples where each tuple contains a weapon name and its `WeaponStats`, sorted by the number of deaths.
fn sort_weapons_by_kills(used_weapons: HashMap<String, WeaponStats>) -> Vec<(String, WeaponStats)> {
    let mut weapon_vec: Vec<_> = used_weapons.into_iter().collect();
    weapon_vec.sort_by(|w1, w2| w2.1.deaths.cmp(&w1.1.deaths).then_with(|| w1.0.cmp(&w2.0)));
    weapon_vec
}

/// Calculates the average distance based on valid distances and total distance.
///
/// # Arguments
///
/// * `valid_distances_count` - The number of valid distances recorded.
/// * `total_distance` - The total distance recorded.
///
/// # Returns
///
/// The average distance, rounded to two decimal places. If there are no valid distances, `0.0` is returned and an error message is printed.
fn calculate_average_distance(valid_distances_count: u32, total_distance: f64) -> f64 {
    if valid_distances_count > 0 {
        (total_distance / valid_distances_count as f64 * 100.0).round() / 100.0
    } else {
        eprintln!("No valid distances.");
        0.0
    }
}
