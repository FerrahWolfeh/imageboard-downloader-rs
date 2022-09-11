use std::io;

use thiserror::Error;

#[allow(clippy::enum_variant_names)]
#[derive(Error, Debug)]
pub enum QueueError {
    #[error("Failed to get current dir from environment")]
    EnvError {
        #[from]
        source: io::Error,
    },

    #[error("Failed to create destination directory. error: {message}")]
    DirCreationError { message: String },

    #[error("Failed to serialize post list: {source}")]
    PostSerializationError {
        #[from]
        source: serde_json::Error,
    },

    #[error("Failed to serialize data into summary file: {error}")]
    BinarySerializeFail { error: String },

    #[error("Failed to deserialize summary file: {error}")]
    SummaryDeserializeError { error: String },

    #[error("Error while adding file to cbz file: {source}")]
    ZipIOError {
        #[from]
        source: zip::result::ZipError,
    },

    #[error("No posts to download!")]
    NoPostsInQueue,
}
