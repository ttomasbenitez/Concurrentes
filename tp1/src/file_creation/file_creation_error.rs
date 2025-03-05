/// Errors that may occur during file creation.
///
/// # Variants
///
/// * `Serialization` - An error occurred while serializing data.
/// * `FileCreation` - An error occurred while creating the file.
/// * `FileWrite` - An error occurred while writing to the file.

#[derive(Debug)]
pub enum FileCreationError {
    Serialization,
    FileCreation,
    FileWrite,
}
