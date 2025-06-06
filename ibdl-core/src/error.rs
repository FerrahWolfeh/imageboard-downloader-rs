//! Defines the primary error type for the `ibdl-core` crate.
//!
//! This module contains [`DownloaderError`](crate::error::DownloaderError), which consolidates various
//! errors that can occur during the imageboard downloading process,
//! including I/O errors, network issues, and problems specific to
//! file handling or CBZ packaging.
use std::{io, num::TryFromIntError};

use ibdl_common::post::error::PostError;
use thiserror::Error;

/// The main error type for operations within the `ibdl-core` crate.
///
/// This enum encompasses a variety of potential issues, from low-level I/O problems
/// to higher-level logic errors related to downloading and file organization.
#[allow(clippy::enum_variant_names)]
#[derive(Error, Debug)]
pub enum DownloaderError {
    /// An error occurred during a file system I/O operation.
    /// Wraps an underlying `std::io::Error`.
    #[error("Failed to access file: {source}")]
    IOError {
        #[from]
        source: io::Error,
    },

    /// Failed to create a required directory, often the output directory or a subdirectory.
    #[error("Failed to create destination directory. error: {message}")]
    DirCreationError { message: String },

    /// An error occurred while serializing summary data (e.g., for a CBZ file).
    #[error("Failed to serialize data into summary file: {error}")]
    SummarySerializeFail { error: String },

    /// An error occurred while deserializing summary data.
    #[error("Failed to deserialize summary file: {error}")]
    SummaryDeserializeFail { error: String },

    /// An error occurred while trying to establish a connection to the download URL for a post.
    /// Wraps an underlying `reqwest::Error`.
    #[error("Failed to connect to download URL: {source}")]
    ConnectionFail {
        #[from]
        source: reqwest::Error,
    },

    /// Indicates that the download queue was initiated with no posts to download.
    #[error("No posts to download!")]
    NoPostsInQueue,

    /// An error occurred while trying to print a line to a progress bar.
    /// This is typically related to UI updates.
    #[error("Failed to print line to Progress Bar: {message}")]
    ProgressBarPrintFail { message: String },

    /// A conversion from a larger integer type to a smaller one failed,
    /// often due to the value being too large to fit.
    #[error("Int conversion failed (maybe size is too large?)")]
    IntConversion(#[from] TryFromIntError),

    /// An error originating from the `ibdl_common::post` module, wrapped for context.
    #[error("Failed to download Post")]
    PostDownloadError(#[from] PostError),

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

    /// The file extension of a post could not be determined or is not recognized/supported.
    #[error("Post has an unknown extension: {message}")]
    UnknownExtension { message: String },

    /// An error specific to CBZ file operations, often related to the `zip` crate.
    /// This variant is only available if the `cbz` feature is enabled.
    #[cfg(feature = "cbz")]
    #[error("Error while adding file to cbz file: {source}")]
    ZipIOError {
        #[from]
        source: zip::result::ZipError,
    },

    /// Failed to obtain exclusive ownership of a `ZipWriter`'s `Arc<Mutex>`.
    /// This usually means that some asynchronous tasks are still holding references to the `Arc`,
    /// preventing the `ZipWriter` from being finalized.
    /// This variant is only available if the `cbz` feature is enabled.
    #[cfg(feature = "cbz")]
    #[error(
        "Failed to get exclusive ownership of ZipWriter Arc. This may indicate that some tasks are still holding references."
    )]
    MutexLockReleaseError,

    /// Failed to spawn or initialize a thread dedicated to writing a file into a ZIP (CBZ) archive.
    /// This variant is only available if the `cbz` feature is enabled.
    #[cfg(feature = "cbz")]
    #[error("Failed to start thread for writing file to destination cbz: {msg}")]
    ZipThreadStartError { msg: String },

    /// An error occurred while writing a file's data into a ZIP (CBZ) archive.
    /// This variant is only available if the `cbz` feature is enabled.
    #[cfg(feature = "cbz")]
    #[error("Failed to write file to destination cbz: {message}")]
    ZipFileWriteError { message: String },

    /// The summary file (often `comics.json` inside a CBZ) was not found or is corrupted.
    /// This variant is only available if the `cbz` feature is enabled.
    #[cfg(feature = "cbz")]
    #[error("Summary file in {file} not found or corrupted")]
    ZipSummaryReadError { file: String },
}
