use serde::{Deserialize, Serialize};

use super::error::ParseErrors;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Rating {
    Safe,
    Questionable,
    Explicit,
    /// Danbooru only
    Gold,
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
            "s" | "safe" | "sensitive" => Ok(Self::Safe),
            "q" | "questionable" => Ok(Self::Questionable),
            "e" | "explicit" => Ok(Self::Explicit),
            "g" => Ok(Self::Gold),
            _ => Err(ParseErrors::RatingParseError(s.to_string())),
        }
    }
}

impl ToString for Rating {
    fn to_string(&self) -> String {
        match self {
            Rating::Safe => String::from("Safe"),
            Rating::Questionable => String::from("Questionable"),
            Rating::Explicit => String::from("Explicit"),
            Rating::Gold => String::from("Gold"),
            Rating::Unknown => String::from("Unknown"),
        }
    }
}

impl Rating {}
