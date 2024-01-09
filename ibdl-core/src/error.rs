use std::{io, num::TryFromIntError};

use ibdl_common::post::error::PostError;
use ibdl_extractors::error::ExtractorError;
use thiserror::Error;

#[allow(clippy::enum_variant_names)]
#[derive(Error, Debug)]
pub enum QueueError {
    #[error("Failed to access file: {source}")]
    IOError {
        #[from]
        source: io::Error,
    },

    #[error("Failed to create destination directory. error: {message}")]
    DirCreationError { message: String },

    #[error("Failed to serialize data into summary file: {error}")]
    SummarySerializeFail { error: String },

    #[error("Failed to deserialize summary file: {error}")]
    SummaryDeserializeFail { error: String },

    #[error("Error while adding file to cbz file: {source}")]
    ZipIOError {
        #[from]
        source: zip::result::ZipError,
    },

    #[error("Summary file in {file} not found or corrupted")]
    ZipSummaryReadError { file: String },

    #[error("No posts to download!")]
    NoPostsInQueue,

    #[error("Failed to print line to Progress Bar: {message}")]
    ProgressBarPrintFail { message: String },

    #[error("Int conversion failed (maybe size is too large?)")]
    IntConversion(#[from] TryFromIntError),

    #[error("Failed to download Post")]
    PostDownloadError(#[from] PostError),
}

#[allow(clippy::enum_variant_names)]
#[derive(Error, Debug)]
pub enum CliError {
    #[error("Failed to authenticate to imageboard: {source}")]
    CoreAuthFail {
        #[from]
        source: ibdl_extractors::auth::Error,
    },

    #[error("Failed to authenticate to imageboard: {source}")]
    InternalExtractorAuthFail {
        #[from]
        source: ExtractorError,
    },

    #[error("Failed to write input to console: {source}")]
    DialoguerIOFail {
        #[from]
        source: dialoguer::Error,
    },

    #[error("Failed to access file: {source}")]
    IOError {
        #[from]
        source: io::Error,
    },

    #[error("Whatever you did, it definetly shouldn't happen...")]
    ImpossibleExecutionPath,

    #[error("This operation is currently unsupported for this imageboard")]
    ExtractorUnsupportedMode,

    #[error("Failed to read server config")]
    ServerConfigSerializeFail,

    #[error("Selected server does not exist.")]
    ServerNotExists,

    #[error("No posts given")]
    NoPostsInInput,
}
