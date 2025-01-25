//! All methods and structs related to user authentication and configuration for imageboard websites
use bincode::serialize;
use ibdl_common::{bincode, log, reqwest, tokio};
use log::debug;
use reqwest::Client;
use std::io;
use std::path::Path;
use thiserror::Error;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

use ibdl_common::ImageBoards;

use ibdl_common::serde::{self, Deserialize, Serialize};

use crate::extractor_config::{ServerConfig, DEFAULT_SERVERS};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AuthState {
    Authenticated,
    NotAuthenticated,
}

impl AuthState {
    #[inline]
    #[must_use]
    pub const fn is_auth(&self) -> bool {
        match self {
            Self::Authenticated => true,
            Self::NotAuthenticated => false,
        }
    }
}

#[derive(Error, Debug)]
pub enum Error {
    /// Indicates that login credentials are incorrect.
    #[error("Invalid username or API key")]
    InvalidLogin,

    /// Indicates errors while connecting or parsing the response from the imageboard.
    #[error("Connection to auth url failed")]
    ConnectionError(#[from] reqwest::Error),

    /// Indicates any unrecoverable IO error when trying to read the auth config file.
    #[error("Failed to read config file. error: {source}")]
    ConfigIOError {
        #[from]
        source: io::Error,
    },

    /// Indicates a failed attempt to serialize the config file to `bincode`.
    #[error("Failed to encode config file")]
    ConfigEncodeError,

    #[error("This imageboard does not support authentication.")]
    AuthUnsupported,
}

/// Struct that defines all user configuration for a specific imageboard.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "self::serde")]
pub struct ImageboardConfig {
    /// Used as a identification tag for handling the cache outside of a imageboard downloader
    /// struct.
    imageboard: ServerConfig,
    pub username: String,
    pub api_key: String,
    pub user_data: UserData,
}

/// Aggregates common user info and it's blacklisted tags in a `AHashSet`.
///
/// It's principally used to filter which posts to download according to the user's blacklist
/// configured in the imageboard profile settings.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "self::serde")]
pub struct UserData {
    pub id: u64,
    pub name: String,
    pub blacklisted_tags: Vec<String>,
}

impl Default for ImageboardConfig {
    fn default() -> Self {
        Self {
            imageboard: DEFAULT_SERVERS.get("danbooru").unwrap().clone(),
            username: String::new(),
            api_key: String::new(),
            user_data: UserData {
                id: 0,
                name: String::new(),
                blacklisted_tags: Vec::new(),
            },
        }
    }
}

impl ImageboardConfig {
    #[must_use]
    pub const fn new(imageboard: ServerConfig, username: String, api_key: String) -> Self {
        Self {
            imageboard,
            username,
            api_key,
            user_data: UserData {
                id: 0,
                name: String::new(),
                blacklisted_tags: Vec::new(),
            },
        }
    }

    pub async fn authenticate(&mut self, client: &Client) -> Result<(), Error> {
        #[derive(Debug, Serialize, Deserialize)]
        #[serde(crate = "self::serde")]
        struct AuthTest {
            pub success: Option<bool>,
            pub id: Option<u64>,
            pub name: Option<String>,
            pub blacklisted_tags: Option<String>,
        }

        if self.imageboard.auth_url.is_none() {
            return Err(Error::AuthUnsupported);
        }

        let url = match self.imageboard.server {
            ImageBoards::Danbooru => self.imageboard.auth_url.as_ref().unwrap().to_string(),
            ImageBoards::E621 => format!(
                "{}{}.json",
                self.imageboard.auth_url.as_ref().unwrap(),
                self.username
            ),
            _ => String::new(),
        };

        debug!("Authenticating to {}", self.imageboard.base_url);

        let req = client
            .get(url)
            .basic_auth(&self.username, Some(&self.api_key))
            .send()
            .await?
            .json::<AuthTest>()
            .await?;

        debug!("{:?}", req);

        if req.success.is_some() {
            return Err(Error::InvalidLogin);
        }

        if req.id.is_some() {
            let tag_list = req.blacklisted_tags.unwrap();

            self.user_data.id = req.id.unwrap();
            self.user_data.name = req.name.unwrap();

            for i in tag_list.lines() {
                if !i.contains("//") {
                    self.user_data.blacklisted_tags.push(i.to_string());
                }
            }

            debug!("User id: {}", self.user_data.id);
            debug!("Blacklisted tags: '{:?}'", self.user_data.blacklisted_tags);

            self.write_cache().await?;
        }

        Ok(())
    }

    /// Generates a bincode file that contains all the data from `self` and saves
    /// it in the directory provided by a `ImageBoards::auth_cache_dir()` method.
    async fn write_cache(&self) -> Result<(), Error> {
        let config_path =
            ImageBoards::auth_cache_dir()?.join(Path::new(&self.imageboard.pretty_name));
        let mut cfg_cache = OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&config_path)
            .await?;

        let Ok(bytes) = serialize(&self) else {
            return Err(Error::ConfigEncodeError);
        };

        cfg_cache.write_all(&bytes).await?;
        debug!("Wrote auth cache to {}", &config_path.display());
        Ok(())
    }
}
