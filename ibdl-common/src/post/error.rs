//! # Post Error Module
//!
//! This module defines the [`PostError`](crate::post::error::PostError) enum, which consolidates all errors
//! that can occur during the processing, downloading, or manipulation of
//! imageboard posts within the `ibdl-common` crate and dependent crates.
//!
//! Each variant represents a specific failure condition, providing context
//! about what went wrong.

use thiserror::Error;

/// Enumerates the possible errors that can arise during post-related operations.
///
/// This error type is used throughout the post handling logic to provide
/// specific information about failures.
#[derive(Error, Debug)]
pub enum PostError {
    /// The file extension of a post could not be determined or is not recognized/supported.
    #[error("Post has an unknown extension: {message}")]
    UnknownExtension { message: String },
}
