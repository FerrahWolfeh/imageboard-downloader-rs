//! General enum for rating posts found by the imageboard downloader
//! # Post Rating
//! In general, most imageboard websites also classify posts considering how explicit they are
//!
//! Posts are usually classified into 4 special tags:
//! * `Safe` or `General`: Posts that don't involve nothing suggestive. Usually normal fanart.
//! * `Questionable` or `Sensitive`: Posts that involve nude/semi-nude characters or other suggestive art that *might* not be safe for viewing close to other people or at work.
//! * `Explicit`: Posts that are explicity pornographic or have other sensitive content such as gore, etc.
//!
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rating {
    /// Represents posts that are don't involve nothing suggestive or sensitive.
    Safe,
    /// Represents posts that have some degree of nudity or sexually suggestive elements.
    Questionable,
    /// Represents posts that have explicit elements of pornography, gore, death, etc.
    Explicit,
    /// Represents a failure to parse the `rating` tag into one of the above.
    Unknown,
}

impl Default for Rating {
    fn default() -> Self {
        Rating::Unknown
    }
}

impl ToString for Rating {
    fn to_string(&self) -> String {
        match self {
            Rating::Safe => String::from("Safe"),
            Rating::Questionable => String::from("Questionable"),
            Rating::Explicit => String::from("Explicit"),
            Rating::Unknown => String::from("Unknown"),
        }
    }
}

#[allow(clippy::should_implement_trait)]
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
