//! # Post Error Module
//!
//! This module defines the [`PostError`](crate::post::error::PostError) enum, which consolidates all errors
//! that can occur during the processing, downloading, or manipulation of
//! imageboard posts within the `ibdl-common` crate and dependent crates.
//!
//! Each variant represents a specific failure condition, providing context
//! about what went wrong.
use std::{io, num::TryFromIntError};

use thiserror::Error;

/// Enumerates the possible errors that can arise during post-related operations.
///
/// This error type is used throughout the post handling logic to provide
/// specific information about failures.
#[derive(Error, Debug)]
pub enum PostError {
    /// Indicates that the file associated with a post already exists locally
    /// and appears to be complete, so no download was attempted.
    #[error("This file was already correctly downloaded")]
    CorrectFileExists,

    /// An error occurred during file I/O operations (e.g., reading, writing, creating).
    /// Wraps an underlying `std::io::Error`.
    #[error("Failed to access file: {source}")]
    FileIOError {
        #[from]
        source: io::Error,
    },

    /// Failed to print a line or update the progress bar display.
    #[error("Failed to print line to Progress Bar: {message}")]
    ProgressBarPrintFail { message: String },

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

    /// A conversion between integer types failed, often due to an out-of-range value
    /// (e.g., trying to convert a large `u64` to `u32`).
    #[error("Int conversion failed (maybe size is too large?)")]
    IntConversion(#[from] TryFromIntError),

    /// The file extension of a post could not be determined or is not recognized/supported.
    #[error("Post has an unknown extension: {message}")]
    UnknownExtension { message: String },
}
