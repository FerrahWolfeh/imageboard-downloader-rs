//! All methods and structs related to user authentication and configuration for imageboard websites
use crate::ImageBoards;
use ahash::AHashSet;
use anyhow::{bail, Error};
use bincode::serialize;
use log::debug;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

/// Struct that defines all user configuration for a specific imageboard.
#[derive(Serialize, Deserialize, Debug)]
pub struct ImageboardConfig {
    /// Used as a identification tag for handling the cache outside of a imageboard downloader
    /// struct.
    imageboard: ImageBoards,
    pub username: String,
    pub api_key: String,
    pub user_data: UserData,
}

/// Aggregates common user info and it's blacklisted tags in a `HashSet`.
///
/// It's principally used to filter which posts to download according to the user's blacklist
/// configured in the imageboard profile settings.
#[derive(Serialize, Deserialize, Debug)]
pub struct UserData {
    pub id: u64,
    pub name: String,
    pub blacklisted_tags: AHashSet<String>,
}

impl Default for ImageboardConfig {
    fn default() -> Self {
        Self {
            imageboard: ImageBoards::Danbooru,
            username: "".to_string(),
            api_key: "".to_string(),
            user_data: UserData {
                id: 0,
                name: "".to_string(),
                blacklisted_tags: AHashSet::new(),
            },
        }
    }
}

impl ImageboardConfig {
    pub fn new(imageboard: ImageBoards, username: String, api_key: String) -> Self {
        Self {
            imageboard,
            username,
            api_key,
            user_data: UserData {
                id: 0,
                name: "".to_string(),
                blacklisted_tags: AHashSet::new(),
            },
        }
    }

    pub async fn authenticate(&mut self, client: &Client) -> Result<(), Error> {
        #[derive(Debug, Serialize, Deserialize)]
        struct AuthTest {
            pub success: Option<bool>,
            pub id: Option<u64>,
            pub name: Option<String>,
            pub blacklisted_tags: Option<String>,
        }

        let url = match self.imageboard {
            ImageBoards::Danbooru => self.imageboard.auth_url().to_string(),
            ImageBoards::E621 => format!("{}{}.json", self.imageboard.auth_url(), self.username),
            _ => String::new(),
        };

        debug!("Authenticating to {}", self.imageboard.to_string());

        let req = client
            .get(url)
            .basic_auth(&self.username, Some(&self.api_key))
            .send()
            .await?
            .json::<AuthTest>()
            .await?;

        debug!("{:?}", req);

        if req.success.is_some() {
            bail!("Invalid username or api key!")
        }

        if req.id.is_some() {
            let tag_list = req.blacklisted_tags.unwrap();

            self.user_data.id = req.id.unwrap();
            self.user_data.name = req.name.unwrap();

            for i in tag_list.lines() {
                if !i.contains("//") {
                    self.user_data.blacklisted_tags.insert(i.to_string());
                }
            }

            debug!("User id: {}", self.user_data.id);
            debug!("Blacklisted tags: '{:?}'", self.user_data.blacklisted_tags);

            Self::write_cache(self).await?;
        }

        Ok(())
    }

    /// Generates a zstd-compressed bincode file that contains all the data from `self` and saves
    /// it in the directory provided by a `ImageBoards::Variant.auth_cache_dir()` method.
    async fn write_cache(&self) -> Result<(), Error> {
        let config_path = self.imageboard.auth_cache_dir()?;
        let mut cfg_cache = OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&config_path)
            .await?;
        let cfg = serialize(&self)?;
        let compressed_data = zstd::encode_all(cfg.as_slice(), 7)?;
        cfg_cache.write_all(&compressed_data).await?;
        debug!("Wrote auth cache to {}", &config_path.display());
        Ok(())
    }
}
