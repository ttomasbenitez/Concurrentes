use crate::data_summary::deaths_info_summary::DeathsInfoSummary;
use serde::Serialize;

/// Wrapper structure for serializing `DeathsInfoSummary` with an additional `padron` field.
///
/// # Fields
///
/// * `padron` - Student's padron.
/// * `deaths_info` - The `DeathsInfoSummary` data, flattened into the JSON object.
#[derive(Serialize, Debug)]
pub struct JsonFormat {
    pub(crate) padron: u32,
    #[serde(flatten)]
    pub(crate) deaths_info: DeathsInfoSummary,
}
