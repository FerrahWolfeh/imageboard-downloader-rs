use std::ops::Deref;
pub use clap;
use clap::ValueEnum;
use ibdl_common::{post::rating::Rating, ImageBoards};
use std::path::{Path, PathBuf};

pub mod cli;
pub mod progress_bars;
pub mod queue;

#[derive(Debug, Clone, Copy)]
pub struct ImageBoardArg {
    pub inner: ImageBoards,
}

#[derive(Debug, Clone, Copy)]
pub struct RatingArg {
    pub inner: Rating,
}

impl ValueEnum for RatingArg {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self {
                inner: Rating::Safe,
            },
            Self {
                inner: Rating::Questionable,
            },
            Self {
                inner: Rating::Explicit,
            },
            Self {
                inner: Rating::Unknown,
            },
        ]
    }
    fn to_possible_value<'a>(&self) -> ::std::option::Option<clap::PossibleValue<'a>> {
        match self.inner {
            Rating::Safe => {
                Some(clap::PossibleValue::new("safe").help(
                    "Represents posts that are don't involve nothing suggestive or sensitive",
                ))
            }
            Rating::Questionable => Some(clap::PossibleValue::new("questionable").help(
                "Represents posts that have some degree of nudity or sexually suggestive elements",
            )),
            Rating::Explicit => Some(clap::PossibleValue::new("explicit").help(
                "Represents posts that have explicit elements of pornography, gore, death, etc",
            )),
            Rating::Unknown => Some(
                clap::PossibleValue::new("unknown")
                    .help("Represents a failure to parse the `rating` tag into one of the above"),
            ),
        }
    }
}

impl Deref for RatingArg {
    type Target = Rating;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl ValueEnum for ImageBoardArg {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self {
                inner: ImageBoards::Danbooru,
            },
            Self {
                inner: ImageBoards::E621,
            },
            Self {
                inner: ImageBoards::Rule34,
            },
            Self {
                inner: ImageBoards::Gelbooru,
            },
            Self {
                inner: ImageBoards::Realbooru,
            },
            Self {
                inner: ImageBoards::Realbooru,
            },
        ]
    }

    fn to_possible_value<'a>(&self) -> ::std::option::Option<clap::PossibleValue<'a>> {
        match self.inner {
            ImageBoards::Danbooru => {
                Some(
                    clap::PossibleValue::new("danbooru")
                        .help(
                            "Represents the website ```https://danbooru.donmai.us``` or it's safe variant ```https://safebooru.donmai.us```",
                        ),
                )
            }
            ImageBoards::E621 => {
                Some(
                    clap::PossibleValue::new("e621")
                        .help(
                            "Represents the website ```https://e621.net``` or it's safe variant ```https://e926.net```",
                        ),
                )
            }
            ImageBoards::Rule34 => {
                Some(
                    clap::PossibleValue::new("rule34")
                        .help("Represents the website ```https://rule34.xxx```"),
                )
            }
            ImageBoards::Realbooru => {
                Some(
                    clap::PossibleValue::new("realbooru")
                        .help("Represents the website ```http://realbooru.com```"),
                )
            }
            ImageBoards::Konachan => {
                Some(
                    clap::PossibleValue::new("konachan")
                        .help(
                            "Represents the website ```https://konachan.com``` or it's safe variant ```https://konachan.net```",
                        ),
                )
            }
            ImageBoards::Gelbooru => {
                Some(
                    clap::PossibleValue::new("gelbooru")
                        .help("Represents the website ```https://gelbooru.com```"),
                )
            }
        }
    }
}

impl Deref for ImageBoardArg {
    type Target = ImageBoards;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[inline]
pub fn generate_output_path(
    main_path: PathBuf,
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
