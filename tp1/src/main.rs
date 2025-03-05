mod data_processing;
mod data_summary;
mod file_creation;

use crate::data_processing::data_processor::process_data_in_parallel;
use crate::file_creation::file_creator::create_json_file;
use std::env;

/// Parses command-line arguments for input path, number of threads, and output file name.
///
/// # Returns
///
/// * `Ok((input_path, num_threads, output_file_name))` - A tuple containing the input path, number of threads, and output file name.
/// * `Err(String)` - An error message if the arguments are invalid or missing.
fn parse_args() -> Result<(String, usize, String), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        return Err("Usage: cargo run <input-path> <num-threads> <output-file-name>".into());
    }

    let input_path = args[1].clone();
    let num_threads: usize = args[2]
        .parse()
        .map_err(|_| "Number of threads must be a valid integer.")?;
    let output_file_name = args[3].clone();

    Ok((input_path, num_threads, output_file_name))
}

fn main() -> Result<(), String> {
    let (input_path, num_threads, output_file_name) = parse_args()?;

    let deaths_info = process_data_in_parallel(&input_path, num_threads);
    match create_json_file(&output_file_name, deaths_info) {
        Ok(()) => println!("File created successfully."),
        Err(err) => {
            eprintln!("Failed to create file: {:?}", err);
        }
    }

    Ok(())
}
