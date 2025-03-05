use crate::data_processing::player_stats::PlayerStats;
use crate::data_processing::weapon_stats::WeaponStats;
use crate::data_summary::data_summarizer::summarize;
use crate::data_summary::deaths_info_summary::DeathsInfoSummary;

use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use rayon::ThreadPoolBuilder;
use std::collections::HashMap;
use std::fs::{read_dir, File};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

/// Processes data in parallel from CSV files in the given directory.
///
/// # Arguments
///
/// * `dir_path` - The directory containing CSV files to process.
/// * `num_threads` - The number of threads to use for parallel processing.
///
/// # Returns
///
/// A `DeathsInfoSummary` containing the summarized deaths information.
pub fn process_data_in_parallel(dir_path: &str, num_threads: usize) -> DeathsInfoSummary {
    summarize(process_directory(dir_path, num_threads))
}

/// Processes CSV files in the specified directory using a thread pool and aggregates player stats.
///
/// This function creates a thread pool with the specified number of threads to process CSV files in parallel. If an error occurs
/// while creating the thread pool or reading the directory, an error message is printed, and an empty `HashMap` is returned.
///
/// # Arguments
///
/// * `dir_path` - The directory containing CSV files to process.
/// * `num_threads` - The number of threads to use for parallel processing.
///
/// # Returns
///
/// A `HashMap` where keys are player names and values are their respective `PlayerStats`.
fn process_directory(dir_path: &str, num_threads: usize) -> HashMap<String, PlayerStats> {
    let thread_pool = match ThreadPoolBuilder::new().num_threads(num_threads).build() {
        Ok(pool) => pool,
        Err(err) => {
            eprintln!("Error creating thread pool: {}", err);
            return HashMap::new();
        }
    };

    let deaths_info = thread_pool.install(|| {
        let paths: Vec<PathBuf> = collect_csv_files(dir_path);

        paths
            .par_iter()
            .map(process_file)
            .reduce(HashMap::new, merge_files_info)
    });

    deaths_info
}

/// Collects all CSV files from the specified directory.
///
/// # Arguments
///
/// * `dir_path` - The directory containing CSV files.
///
/// # Returns
///
/// A `Vec<PathBuf>` containing the paths to all CSV files found in the directory.
fn collect_csv_files(dir_path: &str) -> Vec<PathBuf> {
    match read_dir(dir_path) {
        Ok(dir) => dir
            .flatten()
            .map(|d| d.path())
            .filter(|path| path.extension().map_or(false, |ext| ext == "csv"))
            .collect(),
        Err(err) => {
            eprintln!("Error reading directory {}: {}", dir_path, err);
            Vec::new()
        }
    }
}

/// Processes a single CSV file and aggregates player stats.
///
/// This function reads the specified CSV file, skipping the first line which contains the format definition,
/// and updates player statistics based on the data. If an error occurs while opening the file, an error message
/// is printed, and an empty `HashMap` is returned.
///
/// # Arguments
///
/// * `path` - The path to the CSV file.
///
/// # Returns
///
/// A `HashMap` where keys are player names and values are their respective `PlayerStats`.
fn process_file(path: &PathBuf) -> HashMap<String, PlayerStats> {
    let mut local_deaths_info = HashMap::new();

    match File::open(path) {
        Ok(file) => {
            let reader = BufReader::new(file);
            for line in reader.lines().skip(1) {
                match line {
                    Ok(line_content) => {
                        update_stats_from_line(&line_content, &mut local_deaths_info)
                    }
                    Err(err) => eprintln!("Error reading line in file {}: {}", path.display(), err),
                }
            }
        }
        Err(err) => {
            eprintln!("Error opening file {}: {}", path.display(), err);
        }
    }

    local_deaths_info
}

/// Updates player statistics based on a single line from a CSV file.
///
/// # Arguments
///
/// * `line_content` - The content of a line from the CSV file.
/// * `local_deaths_info` - A mutable reference to the `HashMap` where player statistics are being updated.
fn update_stats_from_line(
    line_content: &str,
    local_deaths_info: &mut HashMap<String, PlayerStats>,
) {
    let mut fields = line_content.split(',');

    if let (Some(weapon_name), Some(killer_name)) = (fields.next(), fields.next()) {
        let player_stats = local_deaths_info
            .entry(killer_name.to_owned())
            .or_insert_with(|| PlayerStats {
                used_weapons: HashMap::new(),
                deaths: 0,
            });
        player_stats.deaths += 1;

        let weapon_stats = player_stats
            .used_weapons
            .entry(weapon_name.to_owned())
            .or_insert_with(|| WeaponStats {
                deaths: 0,
                valid_distances_count: 0,
                total_distance: 0.0,
            });
        weapon_stats.deaths += 1;

        update_weapon_distance_stats(&mut fields, weapon_stats);
    }
}

/// Updates the weapon distance statistics based on remaining fields in a line from a CSV file.
///
/// # Arguments
///
/// * `fields` - The remaining fields from a CSV line, after extracting the weapon and killer name.
/// * `weapon_stats` - A mutable reference to the `WeaponStats` for the current weapon.
fn update_weapon_distance_stats(
    fields: &mut std::str::Split<char>,
    weapon_stats: &mut WeaponStats,
) {
    if let (Some(killer_x_str), Some(killer_y_str), Some(victim_x_str), Some(victim_y_str)) =
        (fields.nth(1), fields.next(), fields.nth(5), fields.next())
    {
        if let Some(distance) =
            calculate_distance(killer_x_str, killer_y_str, victim_x_str, victim_y_str)
        {
            weapon_stats.total_distance += distance;
            weapon_stats.valid_distances_count += 1;
        }
    }
}

/// Calculates the distance between the killer and the victim.
///
/// # Arguments
///
/// * `killer_x_str`, `killer_y_str`, `victim_x_str`, `victim_y_str` - Strings representing the coordinates of the killer and victim.
///
/// # Returns
///
/// An `Option<f64>` representing the distance if all coordinates are valid, otherwise `None`.
fn calculate_distance(
    killer_x_str: &str,
    killer_y_str: &str,
    victim_x_str: &str,
    victim_y_str: &str,
) -> Option<f64> {
    let killer_x: f64 = killer_x_str.parse().ok()?;
    let killer_y: f64 = killer_y_str.parse().ok()?;
    let victim_x: f64 = victim_x_str.parse().ok()?;
    let victim_y: f64 = victim_y_str.parse().ok()?;

    Some(((killer_x - victim_x).powi(2) + (killer_y - victim_y).powi(2)).sqrt())
}

/// Merges local file player stats into the final aggregated stats.
///
/// This function updates the final player statistics by combining them with the player statistics from a local file.
///
/// # Arguments
///
/// * `final_deaths_info` - The final aggregated player stats.
/// * `local_deaths_info` - The player stats from the current file.
///
/// # Returns
///
/// The updated `final_deaths_info` with merged stats.
fn merge_files_info(
    mut final_deaths_info: HashMap<String, PlayerStats>,
    local_deaths_info: HashMap<String, PlayerStats>,
) -> HashMap<String, PlayerStats> {
    local_deaths_info
        .into_iter()
        .for_each(|(player, local_player_stats)| {
            let final_player_stats =
                final_deaths_info
                    .entry(player)
                    .or_insert_with(|| PlayerStats {
                        used_weapons: HashMap::new(),
                        deaths: 0,
                    });

            final_player_stats.deaths += local_player_stats.deaths;
            merge_weapon_stats(
                &mut final_player_stats.used_weapons,
                local_player_stats.used_weapons,
            );
        });
    final_deaths_info
}

/// Merges weapon stats from a local file into the final aggregated weapon stats.
///
/// # Arguments
///
/// * `final_weapons` - A mutable reference to the final aggregated weapon stats.
/// * `local_weapons` - The weapon stats from the current file.
fn merge_weapon_stats(
    final_weapons: &mut HashMap<String, WeaponStats>,
    local_weapons: HashMap<String, WeaponStats>,
) {
    local_weapons
        .into_iter()
        .for_each(|(weapon, local_weapon_stats)| {
            let final_weapon_stats = final_weapons.entry(weapon).or_insert_with(|| WeaponStats {
                deaths: 0,
                valid_distances_count: 0,
                total_distance: 0.0,
            });

            final_weapon_stats.deaths += local_weapon_stats.deaths;
            final_weapon_stats.total_distance += local_weapon_stats.total_distance;
            final_weapon_stats.valid_distances_count += local_weapon_stats.valid_distances_count;
        });
}
