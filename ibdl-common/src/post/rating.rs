//! General enum for rating posts found by the imageboard downloader
//! # Post Rating
//! In general, most imageboard websites also classify posts considering how explicit they are
//!
//! Posts are usually classified into 4 special tags:
//! * `Safe` or `General`: Posts that don't involve anything suggestive. Usually normal fanart.
//! * `Questionable` or `Sensitive`: Posts that involve nude/seminude characters or other suggestive art that *might* not be safe for viewing close to other people or at work.
//! * `Explicit`: Posts that are explicitly pornographic or have other sensitive content such as gore, etc.
//!

use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default,
)]
pub enum Rating {
    /// Represents posts that are don't involve anything suggestive or sensitive.
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
    /// Guess the variant according to the rating tag present in the post
    pub fn from_rating_str(s: &str) -> Self {
        match s {
            "s" | "g" | "safe" | "sensitive" | "general" => Self::Safe,
            "q" | "questionable" => Self::Questionable,
            "e" | "explicit" => Self::Explicit,
            _ => Self::Unknown,
        }
    }
}
