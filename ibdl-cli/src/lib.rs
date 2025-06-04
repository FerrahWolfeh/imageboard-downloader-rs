use std::ops::Deref;

use clap::ValueEnum;
use ibdl_common::{ImageBoards, post::rating::Rating};

pub mod cli;
pub mod error;
pub mod progress_bars;

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct ImageBoardArg(ImageBoards);

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct RatingArg(pub Rating);

impl ValueEnum for RatingArg {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self(Rating::Safe),
            Self(Rating::Questionable),
            Self(Rating::Explicit),
        ]
    }
    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        match self.0 {
            Rating::Safe => {
                Some(clap::builder::PossibleValue::new("safe").help(
                    "Represents posts that are don't involve nothing suggestive or sensitive",
                ))
            }
            Rating::Questionable => Some(clap::builder::PossibleValue::new("questionable").help(
                "Represents posts that have some degree of nudity or sexually suggestive elements",
            )),
            Rating::Explicit => Some(clap::builder::PossibleValue::new("explicit").help(
                "Represents posts that have explicit elements of pornography, gore, death, etc",
            )),
            _ => None,
        }
    }
}

impl Deref for RatingArg {
    type Target = Rating;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
