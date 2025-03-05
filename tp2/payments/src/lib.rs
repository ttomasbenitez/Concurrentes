use serde::Serialize;

pub mod messages;
pub mod payments;

pub const PAYMENTS_PORT: u16 = 8000;

/// Serializes a message into a JSON-encoded byte vector with a newline delimiter.
///
/// This function converts a given message into a JSON string, appends a newline (`\n`),
/// and returns it as a byte vector. If serialization fails, it returns a default
/// error message as a byte vector.
///
/// # Type Parameters
/// * `M` - A type that implements the `Serialize` trait.
///
/// # Arguments
/// * `msg` - The message to serialize and format.
///
/// # Returns
/// A `Vec<u8>` containing the JSON-encoded message followed by a newline, or an
/// error message if serialization fails.
pub fn format_msg<M: Serialize>(msg: M) -> Vec<u8> {
    if let Ok(json) = serde_json::to_string(&msg) {
        [json.as_bytes(), b"\n"].concat()
    } else {
        b"Error formatting message\n".to_vec()
    }
}
