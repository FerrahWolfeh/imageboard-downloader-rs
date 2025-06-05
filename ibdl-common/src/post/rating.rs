//! # Post Rating Module
//!
//! This module defines the [`Rating`] enum, which represents the content safety
//! classification of an imageboard post. Imageboards typically categorize posts
//! based on their explicitness to allow users to filter content.
//!
//! Common classifications include:
//! - **Safe/General**: Content suitable for all audiences.
//! - **Questionable/Sensitive**: Content that may be mildly suggestive, contain nudity,
//!   or themes not suitable for all environments (e.g., work).
//! - **Explicit**: Content that is overtly sexual, violent, or otherwise not safe for work (NSFW).
//!
//! The [`Rating`] enum provides a standardized way to handle these classifications
//! across different imageboard sources.

use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// Represents the content safety rating of an imageboard post.
#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default,
)]
pub enum Rating {
    /// Content is considered safe for all audiences and environments.
    Safe,
    /// Represents posts that have some degree of nudity or sexually suggestive elements.
    Questionable,
    /// Represents posts that have explicit elements of pornography, gore, death, etc.
    Explicit,
    /// Represents a failure to parse the `rating` tag into one of the above.
    #[default]
    Unknown,
}

impl Display for Rating {
    /// Formats the `Rating` variant into a user-friendly string.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Safe => write!(f, "Safe"),
            Self::Questionable => write!(f, "Questionable"),
            Self::Explicit => write!(f, "Explicit"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

impl Rating {
    /// Parses a string representation of a rating into a `Rating` variant.
    ///
    /// This function attempts to map common rating strings (and their abbreviations)
    /// found in imageboard APIs or tags to the corresponding `Rating` enum variant.
    ///
    /// The matching is case-sensitive.
    pub fn from_rating_str(s: &str) -> Self {
        match s {
            "s" | "g" | "safe" | "sensitive" | "general" => Self::Safe,
            "q" | "questionable" => Self::Questionable,
            "e" | "explicit" => Self::Explicit,
            _ => Self::Unknown,
        }
    }
}
