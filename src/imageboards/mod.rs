use crate::progress_bars::BarTemplates;
use anyhow::Error;
use clap::ValueEnum;
use log::debug;
use std::path::PathBuf;
use xdg::BaseDirectories;

pub mod auth;
mod common;
pub mod danbooru;
pub mod e621;
pub mod konachan;
mod macros;
pub mod realbooru;
pub mod rule34;

#[derive(Debug, Copy, Clone, ValueEnum)]
pub enum ImageBoards {
    /// Represents the website ```https://danbooru.donmai.us``` or it's safe variant ```https://safebooru.donmai.us```.
    Danbooru,
    /// Represents the website ```https://e621.net``` or it's safe variant ```https://e926.net```.
    E621,
    Rule34,
    Realbooru,
    /// Represents the website ```https://konachan.com``` or it's safe variant ```https://konachan.net```.
    Konachan,
}

impl ToString for ImageBoards {
    fn to_string(&self) -> String {
        match self {
            ImageBoards::Danbooru => String::from("danbooru"),
            ImageBoards::E621 => String::from("e621"),
            ImageBoards::Rule34 => String::from("rule34"),
            ImageBoards::Realbooru => String::from("realbooru"),
            ImageBoards::Konachan => String::from("konachan"),
        }
    }
}

impl ImageBoards {
    /// Each variant can generate a specific user-agent to connect to the imageboard site.
    ///
    /// It will always follow the version declared inside ```Cargo.toml```
    pub fn user_agent(&self) -> String {
        let app_name = "Rust Imageboard Downloader";
        let variant = match self {
            ImageBoards::Danbooru => " (by danbooru user FerrahWolfeh)",
            ImageBoards::E621 => " (by e621 user FerrahWolfeh)",
            _ => "",
        };
        let ua = format!("{}/{}{}", app_name, env!("CARGO_PKG_VERSION"), variant);
        debug!("Using user-agent: {}", ua);
        ua
    }

    /// Exclusive to ```ImageBoards::Danbooru```.
    ///
    /// Will return ```Some``` with the endpoint for the total post count with given tags. In case it's used with another variant, it returns ```None```.
    ///
    /// The ```safe``` bool will determine if the endpoint directs to ```https://danbooru.donmai.us``` or ```https://safebooru.donmai.us```.
    pub fn post_count_url(&self, safe: bool) -> Option<&str> {
        match self {
            ImageBoards::Danbooru => {
                if safe {
                    Some("https://safebooru.donmai.us/counts/posts.json")
                } else {
                    Some("https://danbooru.donmai.us/counts/posts.json")
                }
            }
            _ => None,
        }
    }

    /// Returns ```Some``` with the endpoint for the post list with their respective tags.
    ///
    /// Will return ```None``` for still unimplemented imageboards.
    ///
    /// ```safe``` works only with ```Imageboards::Danbooru```, ```Imageboards::E621``` and ```Imageboards::Konachan``` since they are the only ones that have a safe variant for now.
    pub fn post_url(&self, safe: bool) -> Option<&str> {
        match self {
            ImageBoards::Danbooru => {
                if safe {
                    Some("https://safebooru.donmai.us/posts.json")
                } else {
                    Some("https://danbooru.donmai.us/posts.json")
                }
            }
            ImageBoards::E621 => {
                if safe {
                    Some("https://e926.net/posts.json")
                } else {
                    Some("https://e621.net/posts.json")
                }
            }
            _ => None,
        }
    }

    /// Returns special-themed progress bars for each variant
    pub fn progress_template(&self) -> BarTemplates {
        match self {
            ImageBoards::Danbooru => BarTemplates::default(),
            ImageBoards::E621 => BarTemplates {
                main: "{spinner:.yellow.bold} {elapsed_precise:.bold} {wide_bar:.blue/white.dim} {percent:.bold}  {pos:.yellow} ({files_sec:.bold} | eta. {eta})",
                download: "{spinner:.blue.bold} {bar:40.yellow/white.dim} {percent:.bold} | {byte_progress:.blue} @ {bytes_per_sec:>13.yellow} (eta. {eta:.blue})",
            },
            ImageBoards::Rule34 => BarTemplates::default(),
            ImageBoards::Realbooru => BarTemplates::default(),
            ImageBoards::Konachan => BarTemplates::default(),
        }
    }

    pub fn auth_url(&self) -> &str {
        match self {
            ImageBoards::Danbooru => "https://danbooru.donmai.us/profile.json",
            ImageBoards::E621 => todo!(),
            ImageBoards::Rule34 => todo!(),
            ImageBoards::Realbooru => todo!(),
            ImageBoards::Konachan => todo!(),
        }
    }

    pub fn auth_cache_dir(&self) -> Result<PathBuf, Error> {
        let xdg_dir = BaseDirectories::with_prefix("imageboard-downloader-rs")?;

        let dir = xdg_dir.place_config_file(self.to_string())?;
        Ok(dir)
    }
}

// impl fmt::Display for ImageBoards {
//     fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
//         fmt.pad(self.as_str())
//     }
// }
//
// impl ImageBoards {
//     pub fn as_str(&self) -> &'static str {
//         IMAGEBOARD_NAMES[*self as usize]
//     }
//
//     fn from_usize(u: usize) -> Option<ImageBoards> {
//         match u {
//             1 => Some(ImageBoards::Danbooru),
//             2 => Some(ImageBoards::E621),
//             3 => Some(ImageBoards::Rule34),
//             4 => Some(ImageBoards::RealBooru),
//             _ => None,
//         }
//     }
// }
//
// impl FromStr for ImageBoards {
//     type Err = ParseError;
//
//     fn from_str(imgboard: &str) -> Result<ImageBoards, Self::Err> {
//         IMAGEBOARD_NAMES
//             .iter()
//             .position(|&name| str::eq_ignore_ascii_case(name, imgboard))
//             .map(|p| ImageBoards::from_usize(p).unwrap())
//             .ok_or(ParseError(()))
//     }
// }
//
// #[allow(missing_copy_implementations)]
// #[derive(Debug, PartialEq)]
// pub struct ParseError(());
//
// impl fmt::Display for ParseError {
//     fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
//         fmt.write_str(IMAGEBOARD_PARSE_ERROR)
//     }
// }
//
// impl error::Error for ParseError {}

#[cfg(test)]
mod tests {
    use crate::imageboards::common::generate_out_dir;
    use crate::{join_tags, ImageBoards};
    use log::debug;
    use std::path::PathBuf;

    #[test]
    fn test_dir_generation() {
        let tags = join_tags!(["kroos_(arknights)", "weapon"]);
        let path = Some(PathBuf::from("./"));

        let out_dir = generate_out_dir(path, &tags, ImageBoards::Danbooru).unwrap();

        assert_eq!(
            PathBuf::from("./danbooru/kroos_(arknights)+weapon"),
            out_dir
        );
    }
}
