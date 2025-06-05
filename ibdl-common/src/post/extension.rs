//! # Post Extension Module
//!
//! This module defines the [`Extension`] enum, which represents the file extension
//! of a media file associated with an imageboard post. It provides utilities for
//! parsing extension strings and determining file characteristics (e.g., if it's a video).
//!
//! The [`Extension`] enum is a crucial part of the [`Post`](crate::post::Post) struct,
//! ensuring that downloaded files are saved with the correct extension.

use std::fmt::{Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::error::PostError; // Assuming PostError is well-documented elsewhere

/// Represents the file extension of a downloaded media file.
///
/// This enum covers common image, video, and special formats found on imageboards.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Extension {
    AVIF,
    JXL,
    /// The `JPG` variant also encompasses the other extensions a jpeg might have, including `.jpg`, `.jpeg` and `.jfif`
    JPG,
    /// The `PNG` variant can also include the rare `.apng` whenever it's present.
    PNG,
    WEBP,
    GIF,
    WEBM,
    MP4,
    /// Pixiv Ugoira is usually downloaded as a zip file with all frames without any additional metadata.
    Ugoira,
    /// Used for any file whose extension is unknown or not currently supported by this library.
    Unknown,
}

impl Extension {
    /// Attempts to determine the `Extension` from a string slice.
    ///
    /// This function parses the input string (case-insensitively) and returns the
    /// corresponding `Extension` variant. If the string does not match any known
    /// extension, it defaults to [`Extension::Unknown`].
    ///
    /// This function will never panic.
    ///
    /// # Examples
    /// ```
    /// # use ibdl_common::post::extension::Extension;
    /// assert_eq!(Extension::guess_format("jpg"), Extension::JPG);
    /// assert_eq!(Extension::guess_format("PNG"), Extension::PNG);
    /// assert_eq!(Extension::guess_format("nonexistent"), Extension::Unknown);
    /// ```
    pub fn guess_format(s: &str) -> Self {
        Self::from_str(s).unwrap_or(Self::Unknown)
    }

    /// Checks if the extension typically represents a video or animated format.
    ///
    /// This includes traditional video formats like MP4 and WEBM, as well as
    /// animated image formats like GIF and the special Ugoira (zip) format.
    pub const fn is_video(&self) -> bool {
        matches!(self, Self::GIF | Self::WEBM | Self::MP4 | Self::Ugoira)
    }
}

impl FromStr for Extension {
    /// The error type returned when parsing an extension string fails.
    type Err = PostError;

    /// Parses a string slice into an `Extension` variant.
    ///
    /// The parsing is case-insensitive. If the string does not correspond to a
    /// known extension, a [`PostError::UnknownExtension`] is returned.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "jpg" | "jpeg" | "jfif" => Ok(Self::JPG),
            "png" | "apng" => Ok(Self::PNG),
            "webp" => Ok(Self::WEBP),
            "webm" => Ok(Self::WEBM),
            "mp4" => Ok(Self::MP4),
            "gif" => Ok(Self::GIF),
            "zip" => Ok(Self::Ugoira),
            "jxl" => Ok(Self::JXL),
            "avif" => Ok(Self::AVIF),
            _ => Err(PostError::UnknownExtension {
                message: s.to_string(),
            }),
        }
    }
}

impl Display for Extension {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JPG => write!(f, "jpg"),
            Self::PNG => write!(f, "png"),
            Self::WEBP => write!(f, "webp"),
            Self::GIF => write!(f, "gif"),
            Self::WEBM => write!(f, "webm"),
            Self::MP4 => write!(f, "mp4"),
            Self::Ugoira => write!(f, "zip"),
            Self::Unknown => write!(f, "bin"),
            Self::AVIF => write!(f, "avif"),
            Self::JXL => write!(f, "jxl"),
        }
    }
}
