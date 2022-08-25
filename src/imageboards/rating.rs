use anyhow::{Error, Ok};
use std::str::FromStr;

pub enum Rating {
    Safe,
    Questionable,
    Explicit,
}

impl FromStr for Rating {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "s" => Ok(Self::Safe),
            "q" => Ok(Self::Questionable),
            "e" => Ok(Self::Explicit),
            _ => Err(todo!()),
        }
    }
}

impl ToString for Rating {
    fn to_string(&self) -> String {
        match self {
            Rating::Safe => String::from("Safe"),
            Rating::Questionable => String::from("Questionable"),
            Rating::Explicit => String::from("Explicit"),
        }
    }
}

impl Rating {}
