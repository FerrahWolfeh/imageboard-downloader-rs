use std::{io, num::TryFromIntError};

use ibdl_common::post::error::PostError;
use thiserror::Error;

#[allow(clippy::enum_variant_names)]
#[derive(Error, Debug)]
pub enum DownloaderError {
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

    #[cfg(feature = "cbz")]
    #[error("Error while adding file to cbz file: {source}")]
    ZipIOError {
        #[from]
        source: zip::result::ZipError,
    },

    /// An error occurred while trying to connect to the download URL for a post.
    /// Wraps an underlying `reqwest::Error`.
    #[error("Failed to connect to download URL: {source}")]
    ConnectionFail {
        #[from]
        source: reqwest::Error,
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

    #[error(
        "Failed to get exclusive ownership of ZipWriter Arc. This may indicate that some tasks are still holding references."
    )]
    MutexLockReleaseError,

    /// Indicates that the file associated with a post already exists locally
    /// and appears to be complete, so no download was attempted.
    #[error("This file was already correctly downloaded")]
    CorrectFileExists,

    /// The URL for the post's file is valid, but the remote server indicated
    /// that the file could not be found (e.g., HTTP 404).
    #[error("Post URL is valid but original file doesn't exist")]
    RemoteFileNotFound,

    /// An error occurred while downloading a chunk of the file.
    #[error("Error while fetching chunk: {message}")]
    ChunkDownloadFail { message: String },

    /// Failed to spawn or initialize a thread dedicated to writing a file into a ZIP (CBZ) archive.
    #[error("Failed to start thread for writing file to destination cbz: {msg}")]
    ZipThreadStartError { msg: String },

    /// An error occurred while writing a file's data into a ZIP (CBZ) archive.
    #[error("Failed to write file to destination cbz: {message}")]
    ZipFileWriteError { message: String },

    /// The file extension of a post could not be determined or is not recognized/supported.
    #[error("Post has an unknown extension: {message}")]
    UnknownExtension { message: String },
}
