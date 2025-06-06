//! # Authentication Module
//!
//! This module provides structures and functions for managing user authentication
//! and configuration related to specific imageboard websites. It allows for
//! storing user credentials, fetching user-specific data like blacklisted tags,
//! and handling the authentication process.
//!
//! Key components include [`ImageboardConfig`](crate::auth::ImageboardConfig) for storing credentials and user data,
//! and [`AuthState`](crate::auth::AuthState) to represent the current authentication status.
use bincode::serialize;
use log::debug;
use reqwest::Client;
use thiserror::Error;

use ibdl_common::ImageBoards;

use serde::{Deserialize, Serialize};

use crate::extractor_config::{ServerConfig, DEFAULT_SERVERS};

/// Represents the authentication status of a user for an imageboard.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AuthState {
    /// The user is successfully authenticated.
    Authenticated,
    /// The user is not authenticated, or authentication has failed.
    NotAuthenticated,
}

impl AuthState {
    /// Checks if the current state is `Authenticated`.
    ///
    /// Returns `true` if authenticated, `false` otherwise.
    #[inline]
    #[must_use]
    pub const fn is_auth(&self) -> bool {
        match self {
            Self::Authenticated => true,
            Self::NotAuthenticated => false,
        }
    }
}

/// Errors that can occur during the authentication process or configuration handling.
#[derive(Error, Debug)]
pub enum Error {
    /// Indicates that login credentials are incorrect.
    #[error("Invalid username or API key")]
    InvalidLogin,

    /// Indicates errors while connecting or parsing the response from the imageboard.
    #[error("Connection to auth url failed")]
    ConnectionError(#[from] reqwest::Error),

    /// Indicates a failed attempt to serialize the config file to `bincode`.
    #[error("Failed to encode config file")]
    ConfigEncodeError,

    /// Indicates that the selected imageboard does not support authentication through this mechanism.
    #[error("This imageboard does not support authentication.")]
    AuthUnsupported,
}

/// Struct that defines all user configuration for a specific imageboard.
///
/// It holds the server configuration, user credentials (username and API key),
/// and fetched user data like ID, name, and blacklisted tags.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImageboardConfig {
    /// The `ServerConfig` for the imageboard this configuration applies to.
    /// This provides details like API endpoints and server-specific settings.
    imageboard: ServerConfig,
    /// The username for the imageboard account.
    pub username: String,
    /// The API key associated with the username.
    pub api_key: String,
    /// Data fetched from the user's profile after successful authentication,
    /// including user ID, name, and blacklisted tags.
    pub user_data: UserData,
}

/// Aggregates common user info and it's blacklisted tags in a `AHashSet`.
///
/// It's principally used to filter which posts to download according to the user's blacklist
/// configured in the imageboard profile settings.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserData {
    /// The unique numerical ID of the user on the imageboard.
    pub id: u64,
    /// The display name of the user on the imageboard.
    pub name: String,
    /// A list of tags that the user has blacklisted on their imageboard profile.
    /// These are typically fetched during authentication.
    pub blacklisted_tags: Vec<String>,
}

impl Default for ImageboardConfig {
    /// Creates a default `ImageboardConfig`.
    ///
    /// By default, this initializes with the configuration for "danbooru"
    /// (if available in `DEFAULT_SERVERS`) and empty user credentials/data.
    /// This relies on the "danbooru" feature being enabled for `DEFAULT_SERVERS` to contain it.
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
    /// Creates a new `ImageboardConfig` with the given server configuration, username, and API key.
    ///
    /// User data is initialized to default empty values and should be populated via `authenticate`.
    ///
    /// # Arguments
    /// * `imageboard`: The `ServerConfig` for the target imageboard.
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

    /// Returns the user-friendly "pretty name" of the imageboard server
    /// associated with this configuration.
    #[must_use]
    pub fn server_pretty_name(&self) -> &str {
        &self.imageboard.pretty_name
    }

    /// Attempts to authenticate the user with the imageboard and fetch user data.
    ///
    /// This method sends a request to the imageboard's authentication endpoint
    /// using the stored username and API key. If successful, it populates the
    /// `user_data` field with the fetched user ID, name, and blacklisted tags.
    ///
    /// # Arguments
    /// * `client`: A `reqwest::Client` to use for making the HTTP request.
    ///
    /// # Errors
    /// Returns an `Error` if authentication fails (e.g., invalid credentials, connection issues),
    /// or if the imageboard does not support authentication via this method.
    pub async fn authenticate(&mut self, client: &Client) -> Result<(), Error> {
        #[derive(Debug, Serialize, Deserialize)]
        struct AuthTest {
            pub success: Option<bool>,
            pub id: Option<u64>,
            pub name: Option<String>,
            pub blacklisted_tags: Option<String>,
        }

        // Check if the server config has an authentication URL defined.
        if self.imageboard.auth_url.is_none() {
            return Err(Error::AuthUnsupported);
        }

        let url = match self.imageboard.server {
            ImageBoards::Danbooru => self.imageboard.auth_url.as_ref().unwrap().to_string(),
            ImageBoards::E621 => format!(
                // E621 auth URL typically requires the username in the path.
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

        debug!("{req:?}");

        // Danbooru returns `success: false` on invalid login.
        if req.success.is_some() {
            return Err(Error::InvalidLogin);
        }

        // E621 returns user data directly on success, or an error structure on failure (handled by reqwest error).
        if req.id.is_some() {
            let tag_list = req.blacklisted_tags.unwrap();

            self.user_data.id = req.id.unwrap();
            self.user_data.name = req.name.unwrap();

            for i in tag_list.lines() {
                // Assuming blacklisted tags are newline-separated and comments start with //
                if !i.contains("//") {
                    self.user_data.blacklisted_tags.push(i.to_string());
                }
            }

            debug!("User id: {}", self.user_data.id);
            debug!("Blacklisted tags: '{:?}'", self.user_data.blacklisted_tags);

            // Note: Caching/writing of this updated config is handled externally.
        }

        Ok(())
    }

    /// Serializes the `ImageboardConfig` into bincode-encoded bytes.
    ///
    /// This allows external code to handle the actual writing of the cache,
    /// making the caching process IO-agnostic within this struct.
    ///
    /// # Errors
    /// Returns `Error::ConfigEncodeError` if serialization fails.
    pub fn to_bincode_bytes(&self) -> Result<Vec<u8>, Error> {
        serialize(&self).map_err(|_| Error::ConfigEncodeError)
    }
}
