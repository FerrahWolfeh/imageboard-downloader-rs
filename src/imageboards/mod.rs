use crate::progress_bars::BarTemplates;
use crate::ImageboardConfig;
use anyhow::Error;
use bincode::deserialize;
use clap::ValueEnum;
use log::{debug, error, warn};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use colored::Colorize;
use tokio::fs::{read, remove_file};
use xdg::BaseDirectories;

pub mod auth;
mod common;
pub mod danbooru;
pub mod e621;
pub mod konachan;
mod macros;
pub mod realbooru;
pub mod rule34;

#[derive(Debug, Copy, Clone, ValueEnum, Serialize, Deserialize)]
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
            _ => None,
        }
    }

    /// Returns special-themed progress bars for each variant
    pub fn progress_template(self) -> BarTemplates {
        match self {
            ImageBoards::E621 => BarTemplates {
                main: "{spinner:.yellow.bold} {elapsed_precise:.bold} {wide_bar:.blue/white.dim} {percent:.bold}  {pos:.yellow} ({files_sec:.bold} | eta. {eta})",
                download: "{spinner:.blue.bold} {bar:40.yellow/white.dim} {percent:.bold} | {byte_progress:.blue} @ {bytes_per_sec:>13.yellow} (eta. {eta:.blue})",
            },
            ImageBoards::Danbooru | ImageBoards::Rule34 | ImageBoards::Realbooru | ImageBoards::Konachan => BarTemplates::default(),
        }
    }

    pub fn auth_url(self) -> &'static str {
        match self {
            ImageBoards::Danbooru => "https://danbooru.donmai.us/profile.json",
            ImageBoards::E621 => "https://e621.net/users/",
            ImageBoards::Rule34 | ImageBoards::Realbooru | ImageBoards::Konachan => todo!(),
        }
    }

    pub fn auth_cache_dir(self) -> Result<PathBuf, Error> {
        let xdg_dir = BaseDirectories::with_prefix("imageboard-downloader")?;

        let dir = xdg_dir.place_config_file(self.to_string())?;
        Ok(dir)
    }

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
                    warn!("{}", "Auth cache is invalid or empty. Running without authentication");
                    Ok(None)
                };
            } else {
                debug!("Failed to decompress authentication cache.");
                debug!("Removing corrupted file");
                remove_file(self.auth_cache_dir()?).await?;
                error!("{}", "Auth cache is corrupted. Please authenticate again.".bold().red());
            }
        };
        debug!("Running without authentication");
        Ok(None)
    }
}

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
