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

use super::error::ParseErrors;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Rating {
    Safe,
    Questionable,
    Explicit,
    Unknown,
}

impl Default for Rating {
    fn default() -> Self {
        Rating::Unknown
    }
}

impl FromStr for Rating {
    type Err = ParseErrors;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "s" | "g" | "safe" | "sensitive" | "general" => Ok(Self::Safe),
            "q" | "questionable" => Ok(Self::Questionable),
            "e" | "explicit" => Ok(Self::Explicit),
            _ => Ok(Self::Unknown),
        }
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

impl Rating {}
