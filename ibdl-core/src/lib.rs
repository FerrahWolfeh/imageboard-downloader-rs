pub use clap;
use clap::ValueEnum;
use ibdl_common::{post::rating::Rating, ImageBoards};
pub use owo_colors;
use std::ops::Deref;
use std::path::{Path, PathBuf};

pub mod async_queue;
pub mod cli;
pub mod progress_bars;
pub mod queue;

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

impl ValueEnum for ImageBoardArg {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self(ImageBoards::Danbooru),
            Self(ImageBoards::E621),
            Self(ImageBoards::Rule34),
            Self(ImageBoards::Gelbooru),
            Self(ImageBoards::Realbooru),
            Self(ImageBoards::Konachan),
        ]
    }

    fn to_possible_value(&self) -> ::std::option::Option<clap::builder::PossibleValue> {
        match self.0 {
            ImageBoards::Danbooru => {
                Some(
                    clap::builder::PossibleValue::new("danbooru")
                        .help(
                            "Represents the website ```https://danbooru.donmai.us``` or it's safe variant ```https://safebooru.donmai.us```",
                        ),
                )
            }
            ImageBoards::E621 => {
                Some(
                    clap::builder::PossibleValue::new("e621")
                        .help(
                            "Represents the website ```https://e621.net``` or it's safe variant ```https://e926.net```",
                        ),
                )
            }
            ImageBoards::Rule34 => {
                Some(
                    clap::builder::PossibleValue::new("rule34")
                        .help("Represents the website ```https://rule34.xxx```"),
                )
            }
            ImageBoards::Realbooru => {
                Some(
                    clap::builder::PossibleValue::new("realbooru")
                        .help("Represents the website ```http://realbooru.com```"),
                )
            }
            ImageBoards::Konachan => {
                Some(
                    clap::builder::PossibleValue::new("konachan")
                        .help(
                            "Represents the website ```https://konachan.com``` or it's safe variant ```https://konachan.net```",
                        ),
                )
            }
            ImageBoards::Gelbooru => {
                Some(
                    clap::builder::PossibleValue::new("gelbooru")
                        .help("Represents the website ```https://gelbooru.com```"),
                )
            }
        }
    }
}

impl Deref for ImageBoardArg {
    type Target = ImageBoards;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[inline]
pub fn generate_output_path(
    main_path: &Path,
    imageboard: ImageBoards,
    tags: &[String],
    cbz_mode: bool,
) -> PathBuf {
    let tag_string = tags.join(" ");
    let tag_path_string = if tag_string.contains("fav:") {
        String::from("Favorites")
    } else if cfg!(windows) {
        tag_string.replace(':', "_")
    } else {
        tag_string
    };

    let pbuf = main_path.join(Path::new(&imageboard.to_string()));

    if cbz_mode {
        return pbuf.join(Path::new(&format!("{}.cbz", tag_path_string)));
    }
    pbuf.join(Path::new(&tag_path_string))
}

/// This function creates the destination directory without creating additional ones related to
/// the selected imageboard or tags used.
#[inline]
pub fn generate_output_path_precise(main_path: &Path, cbz_mode: bool) -> PathBuf {
    if cbz_mode {
        return PathBuf::from(&format!("{}.cbz", main_path.display()));
    }
    main_path.to_path_buf()
}
