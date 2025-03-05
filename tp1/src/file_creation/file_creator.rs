use crate::data_summary::deaths_info_summary::DeathsInfoSummary;
use crate::file_creation::file_creation_error::FileCreationError;
use crate::file_creation::json_format::JsonFormat;

use std::fs::File;
use std::io::BufWriter;
use std::io::Write;

const PADRON: u32 = 106841;

/// Serializes `DeathsInfoSummary` into a JSON string with a given `padron`.
///
/// # Arguments
///
/// * `padron` - Student's padron.
/// * `deaths_info` - The `DeathsInfoSummary` data to be serialized.
///
/// # Returns
///
/// A `Result` containing the serialized JSON string if successful, or a `FileCreationError` if serialization fails.
fn generate_json(padron: u32, deaths_info: DeathsInfoSummary) -> Result<String, FileCreationError> {
    let wrapper = JsonFormat {
        padron,
        deaths_info,
    };
    serde_json::to_string_pretty(&wrapper).map_err(|_| FileCreationError::Serialization)
}

/// Creates a JSON file from `DeathsInfoSummary` and writes it to the specified filename.
///
/// # Arguments
///
/// * `filename` - The path to the file where the JSON will be written.
/// * `deaths_info` - The `DeathsInfoSummary` data to be included in the JSON file.
///
/// # Returns
///
/// A `Result` indicating success or failure. Errors are represented by `FileCreationError`.
pub fn create_json_file(
    filename: &str,
    deaths_info: DeathsInfoSummary,
) -> Result<(), FileCreationError> {
    let json_output = generate_json(PADRON, deaths_info)?;

    let file = File::create(filename).map_err(|_| FileCreationError::FileCreation)?;
    let mut writer = BufWriter::new(file);
    writer
        .write_all(json_output.as_bytes())
        .map_err(|_| FileCreationError::FileWrite)?;

    Ok(())
}
