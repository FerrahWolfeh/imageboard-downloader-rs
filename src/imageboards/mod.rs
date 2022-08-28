//! All internal logic for interacting with and downloading from imageboard websites.
use crate::imageboards::auth::ImageboardConfig;
use crate::progress_bars::BarTemplates;
use anyhow::Error;
use bincode::deserialize;
use clap::ValueEnum;
use colored::Colorize;
use log::{debug, error, warn};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs::{read, remove_file};
use xdg::BaseDirectories;

pub mod auth;
#[cfg(feature = "global_blacklist")]
pub mod blacklist;
mod common;
pub mod post;
pub mod queue;
pub mod rating;
pub mod extractors;

/// All currently supported imageboards and their underlying attributes
#[derive(Debug, Copy, Clone, Ord, PartialOrd, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum ImageBoards {
    /// Represents the website ```https://danbooru.donmai.us``` or it's safe variant ```https://safebooru.donmai.us```.
    Danbooru,
    /// Represents the website ```https://e621.net``` or it's safe variant ```https://e926.net```.
    E621,
    /// Represents the website ```https://rule34.xxx```
    Rule34,
    /// Represents the website ```http://realbooru.com```
    Realbooru,
    /// Represents the website ```https://konachan.com``` or it's safe variant ```https://konachan.net```.
    Konachan,
    /// Represents the website ```https://gelbooru.com```.
    Gelbooru,
}

impl ToString for ImageBoards {
    fn to_string(&self) -> String {
        match self {
            ImageBoards::Danbooru => String::from("danbooru"),
            ImageBoards::E621 => String::from("e621"),
            ImageBoards::Rule34 => String::from("rule34"),
            ImageBoards::Realbooru => String::from("realbooru"),
            ImageBoards::Konachan => String::from("konachan"),
            ImageBoards::Gelbooru => String::from("gelbooru"),
        }
    }
}

impl ImageBoards {
    /// Each variant can generate a specific user-agent to connect to the imageboard site.
    ///
    /// It will always follow the version declared inside ```Cargo.toml```
    pub fn user_agent(self) -> String {
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
    pub fn post_count_url(self, safe: bool) -> Option<&'static str> {
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
            ImageBoards::Rule34 => {
                Some("https://api.rule34.xxx/index.php?page=dapi&s=post&q=index&json=1")
            }
            ImageBoards::Konachan => {
                if safe {
                    Some("https://konachan.net/post.json")
                } else {
                    Some("https://konachan.com/post.json")
                }
            }
            ImageBoards::Realbooru => {
                Some("http://realbooru.com/index.php?page=dapi&s=post&q=index&json=1")
            }
            ImageBoards::Gelbooru => Some("http://gelbooru.com/index.php?page=dapi&s=post&q=index&json=1"),
        }
    }

    /// Returns max number of posts per page a imageboard can have
    pub fn max_post_limit(self) -> usize {
        match self {
            ImageBoards::Danbooru => 200,
            ImageBoards::E621 => 320,
            ImageBoards::Rule34 => 1000,
            ImageBoards::Realbooru => 1000,
            ImageBoards::Konachan => 100,
            ImageBoards::Gelbooru => 100,
        }
    }

    /// Returns special-themed progress bar templates for each variant
    pub fn progress_template(self) -> BarTemplates {
        match self {
            ImageBoards::E621 => BarTemplates {
                main: "{spinner:.yellow.bold} {elapsed_precise:.bold} {wide_bar:.blue/white.dim} {percent:.bold}  {pos:.yellow} (eta. {eta})",
                download: "{spinner:.blue.bold} {bar:40.yellow/white.dim} {percent:.bold} | {byte_progress:.blue} @ {bytes_per_sec:>13.yellow} (eta. {eta:.blue})",
            },
            ImageBoards::Realbooru => BarTemplates { 
                main: "{spinner:.red.bold} {elapsed_precise:.bold} {wide_bar:.red/white.dim} {percent:.bold}  {pos:.bold} (eta. {eta})", 
                download: "{spinner:.red.bold} {bar:40.red/white.dim} {percent:.bold} | {byte_progress:.bold.green} @ {bytes_per_sec:>13.red} (eta. {eta})",
            },
            _ => BarTemplates::default(),
        }
    }

    /// Returns the url used for validating the login input and parsing the user`s profile.
    pub fn auth_url(self) -> &'static str {
        match self {
            ImageBoards::Danbooru => "https://danbooru.donmai.us/profile.json",
            ImageBoards::E621 => "https://e621.net/users/",
            _ => todo!(),
        }
    }

    /// Returns a `PathBuf` pointing to the imageboard`s authentication cache.
    ///
    /// This is XDG-compliant and saves cache files to
    /// `$XDG_CONFIG_HOME/imageboard-downloader/<imageboard>`
    pub fn auth_cache_dir(self) -> Result<PathBuf, Error> {
        let xdg_dir = BaseDirectories::with_prefix("imageboard-downloader")?;

        let dir = xdg_dir.place_config_file(self.to_string())?;
        Ok(dir)
    }

    /// Reads and parses the authentication cache from the path provided by `auth_cache_dir`.
    ///
    /// Returns `None` if the file is corrupted or does not exist.
    pub async fn read_config_from_fs(&self) -> Result<Option<ImageboardConfig>, Error> {
        if let Ok(config_auth) = read(self.auth_cache_dir()?).await {
            debug!("Authentication cache found");

            if let Ok(decompressed) = zstd::decode_all(config_auth.as_slice()) {
                debug!("Authentication cache decompressed.");
                return if let Ok(rd) = deserialize::<ImageboardConfig>(&decompressed) {
                    debug!("Authentication cache decoded.");
                    debug!("User id: {}", rd.user_data.id);
                    debug!("Username: {}", rd.user_data.name);
                    debug!("Blacklisted tags: '{:?}'", rd.user_data.blacklisted_tags);
                    Ok(Some(rd))
                } else {
                    warn!(
                        "{}",
                        "Auth cache is invalid or empty. Running without authentication"
                    );
                    Ok(None)
                };
            } else {
                debug!("Failed to decompress authentication cache.");
                debug!("Removing corrupted file");
                remove_file(self.auth_cache_dir()?).await?;
                error!(
                    "{}",
                    "Auth cache is corrupted. Please authenticate again."
                        .bold()
                        .red()
                );
            }
        };
        debug!("Running without authentication");
        Ok(None)
    }
}
