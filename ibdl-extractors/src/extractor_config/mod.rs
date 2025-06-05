//! # Extractor Configuration Module
//!
//! This module centralizes the configuration for various imageboard servers supported
//! by the `ibdl-extractors` crate. It defines the `ServerConfig` struct, which holds
//! all necessary parameters for an extractor to interact with a specific imageboard,
//! such as API endpoints, user-agent strings, and post limits.
//!
//! It also provides a default set of configurations for commonly used imageboards,
//! accessible via the `DEFAULT_SERVERS` static map.
use crate::extractor::caps::ExtractorFeatures;
use crate::extractor::SiteApi;
#[cfg(feature = "danbooru")]
use crate::imageboards::danbooru::DanbooruApi;
#[cfg(feature = "e621")]
use crate::imageboards::e621::E621Api;
#[cfg(feature = "gelbooru")]
use crate::imageboards::gelbooru::GelbooruApi;
#[cfg(feature = "moebooru")]
use crate::imageboards::prelude::MoebooruApi;
use crate::server_config;
use ibdl_common::ImageBoards;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;

/// Default User-Agent string used by extractors when no site-specific UA is defined.
pub(crate) const DEFAULT_EXT_UA: &str =
    concat!("Rust Imageboard Post Extractor/", env!("CARGO_PKG_VERSION"));

/// Default User-Agent string used by the CLI or other frontend applications
/// when no site-specific UA is defined.
pub(crate) const DEFAULT_CLI_UA: &str =
    concat!("Rust Imageboard Downloader/", env!("CARGO_PKG_VERSION"));

// User-Agent strings specifically for Danbooru, including a reference to the maintainer's username.
// These are used if the "danbooru" feature is enabled.
#[cfg(feature = "danbooru")]
pub(crate) const DB_EXT_UA: &str = concat!(
    "Rust Imageboard Post Extractor/",
    env!("CARGO_PKG_VERSION"),
    " (by Danbooru user FerrahWolfeh)"
);

#[cfg(feature = "danbooru")]
pub(crate) const DB_CLI_UA: &str = concat!(
    "Rust Imageboard Downloader/",
    env!("CARGO_PKG_VERSION"),
    " (by Danbooru user FerrahWolfeh)"
);

#[cfg(feature = "e621")]
pub(crate) const E621_CLI_UA: &str = concat!(
    "Rust Imageboard Downloader/",
    env!("CARGO_PKG_VERSION"),
    " (by e621 user FerrahWolfeh)"
);

#[cfg(feature = "e621")]
pub(crate) const E621_EXT_UA: &str = concat!(
    "Rust Imageboard Post Extractor/",
    env!("CARGO_PKG_VERSION"),
    " (by e621 user FerrahWolfeh)"
);

#[cfg(feature = "gelbooru")]
pub(crate) const GB_CLI_UA: &str = concat!(
    "Rust Imageboard Downloader/",
    env!("CARGO_PKG_VERSION"),
    " (by gelbooru user FerrahWolfeh)"
);

#[cfg(feature = "gelbooru")]
pub(crate) const GB_EXT_UA: &str = concat!(
    "Rust Imageboard Post Extractor/",
    env!("CARGO_PKG_VERSION"),
    " (by gelbooru user FerrahWolfeh)"
);

/// Contains macros for simplifying server configuration definitions.
pub mod macros;
/// Handles serialization aspects, potentially for custom server configurations.
pub mod serialize;

/// A lazily initialized static map holding default `ServerConfig` instances for
/// various supported imageboards.
///
/// The keys of the map are short string identifiers for the imageboards (e.g., "danbooru", "e621").
/// This map is populated based on the enabled feature flags (e.g., "danbooru", "e621", "gelbooru", "moebooru").
/// It utilizes the `server_config!` macro for concise configuration definitions.
pub static DEFAULT_SERVERS: Lazy<HashMap<String, ServerConfig>> = Lazy::new(|| {
    let mut hmap = HashMap::new(); // Initialize with default capacity, will grow as needed

    #[cfg(feature = "danbooru")]
    hmap.insert(
        "danbooru".to_string(),
        server_config!(
            "danbooru",
            "Danbooru",
            ImageBoards::Danbooru,
            DB_CLI_UA,
            DB_EXT_UA,
            "https://danbooru.donmai.us",
            Some(String::from("https://danbooru.donmai.us/posts/")),
            "https://danbooru.donmai.us/posts.json",
            Some(String::from("https://danbooru.donmai.us/pools")),
            200,
            Some(String::from("https://danbooru.donmai.us/profile.json")),
            None
        ),
    );

    #[cfg(feature = "e621")]
    hmap.insert(
        "e621".to_string(),
        server_config!(
            "e621",
            "e621",
            ImageBoards::E621,
            E621_CLI_UA,
            E621_EXT_UA,
            "https://e621.net",
            Some(String::from("https://e621.net/posts/")),
            "https://e621.net/posts.json",
            Some(String::from("https://e621.net/pools")),
            320,
            Some(String::from("https://e621.net/users/")),
            None
        ),
    );

    #[cfg(feature = "gelbooru")]
    hmap.insert(
        "gelbooru".to_string(),
        server_config!(
            "gelbooru",
            "Gelbooru",
            ImageBoards::Gelbooru,
            GB_CLI_UA,
            GB_EXT_UA,
            "https://gelbooru.com",
            Some(String::from(
                "https://gelbooru.com/index.php?page=dapi&s=post&q=index&json=1"
            )),
            "https://gelbooru.com/index.php?page=dapi&s=post&q=index&json=1",
            None,
            100,
            None,
            None
        ),
    );

    #[cfg(feature = "gelbooru")] // Rule34 uses Gelbooru-like API
    hmap.insert(
        "rule34".to_string(),
        server_config!(
            "rule34",
            "Rule34",
            ImageBoards::Gelbooru,
            DEFAULT_CLI_UA,
            DEFAULT_EXT_UA,
            "https://rule34.xxx",
            Some(String::from(
                "https://api.rule34.xxx/index.php?page=dapi&s=post&q=index&json=1"
            )),
            "https://api.rule34.xxx/index.php?page=dapi&s=post&q=index&json=1",
            None,
            1000,
            None,
            None
        ),
    );

    #[cfg(feature = "gelbooru")] // Realbooru uses Gelbooru-like API
    hmap.insert(
        "realbooru".to_string(),
        server_config!(
            "realbooru",
            "Realbooru",
            ImageBoards::GelbooruV0_2,
            DEFAULT_CLI_UA,
            DEFAULT_EXT_UA,
            "https://realbooru.com",
            Some(String::from(
                "http://realbooru.com/index.php?page=dapi&s=post&q=index&json=1"
            )),
            "http://realbooru.com/index.php?page=dapi&s=post&q=index&json=1",
            None,
            1000,
            None,
            None
        ),
    );

    #[cfg(feature = "moebooru")]
    hmap.insert(
        "konachan".to_string(),
        server_config!(
            "konachan",
            "Konachan",
            ImageBoards::Moebooru,
            DEFAULT_CLI_UA,
            DEFAULT_EXT_UA,
            "https://konachan.com",
            None,
            "https://konachan.com/post.json",
            None,
            100,
            None,
            None
        ),
    );
    hmap
});

/// Holds all the necessary configuration details for an imageboard server.
///
/// This struct is used by extractors to understand how to interact with a specific
/// imageboard's API, including URLs, user-agents, and supported features.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// A short, unique string identifier for the server (e.g., "danbooru", "e621").
    /// This is often used as a key in configuration maps.
    pub name: String,
    /// A user-friendly, displayable name for the imageboard (e.g., "Danbooru", "e621").
    pub pretty_name: String,
    /// The `ImageBoards` enum variant that categorizes this server type.
    pub server: ImageBoards,
    /// The User-Agent string to be used by client applications (like a CLI downloader)
    /// when interacting with this server.
    pub client_user_agent: String,
    /// The User-Agent string to be used by the extractor logic itself when making API requests.
    pub extractor_user_agent: String,
    /// The base URL of the imageboard (e.g., "<https://danbooru.donmai.us>").
    pub base_url: String,
    /// An optional URL template for accessing individual posts directly, often including a placeholder for the post ID.
    pub post_url: Option<String>,
    /// The primary API endpoint URL for fetching lists of posts (e.g., a JSON endpoint).
    pub post_list_url: Option<String>,
    /// An optional API endpoint URL for fetching information about pools or sets of posts.
    pub pool_idx_url: Option<String>,
    /// The maximum number of posts the server's API can return in a single request.
    pub max_post_limit: u16,
    /// An optional URL used for authentication checks or fetching user profile information.
    pub auth_url: Option<String>,
    /// An optional, site-specific URL pattern or base for constructing direct image URLs.
    /// This field might be used by specific extractors if the primary `post.url` is not direct.
    pub image_url: Option<String>,
}

impl ServerConfig {
    /// Retrieves the `ExtractorFeatures` for the server type defined in this configuration.
    ///
    /// This method maps the `self.server` (an `ImageBoards` variant) to the
    /// specific features supported by its corresponding extractor implementation.
    ///
    /// # Panics
    /// Panics if the feature flag corresponding to `self.server` (e.g., "danbooru" for `ImageBoards::Danbooru`)
    /// is not enabled during compilation. This ensures that an attempt is not made to get features
    /// for an extractor that hasn't been compiled into the library.
    #[inline]
    #[must_use]
    pub fn extractor_features(&self) -> ExtractorFeatures {
        match self.server {
            #[cfg(feature = "danbooru")]
            ImageBoards::Danbooru => DanbooruApi::features(),
            #[cfg(feature = "e621")]
            ImageBoards::E621 => E621Api::features(),
            #[cfg(feature = "moebooru")]
            ImageBoards::Moebooru => MoebooruApi::features(),
            #[cfg(feature = "gelbooru")]
            ImageBoards::Gelbooru => GelbooruApi::features(),
            // This arm handles cases where a ServerConfig exists for an ImageBoard variant
            // whose corresponding feature (and thus its Extractor and specific match arm above)
            // is not compiled in. This can happen if a ServerConfig is manually created
            // or if the library is used in an unexpected way with feature flags.
            _ => panic!(
                "Attempted to get extractor features for {:?} \
                 but its corresponding feature flag is not enabled. \
                 Please ensure the required feature (danbooru, e621, gelbooru, or moebooru) is active.",
                self.server
            ),
        }
    }
}

impl Default for ServerConfig {
    /// Provides a default `ServerConfig`.
    ///
    /// By default, this returns the configuration for Danbooru, assuming the "danbooru"
    /// feature is enabled. If not, the User-Agent strings might fall back to defaults.
    fn default() -> Self {
        Self {
            name: String::from("danbooru"),
            pretty_name: String::from("Danbooru"),
            server: ImageBoards::Danbooru,
            client_user_agent: DB_CLI_UA.to_string(),
            extractor_user_agent: DB_EXT_UA.to_string(),
            base_url: String::from("https://danbooru.donmai.us"),
            post_url: Some(String::from("https://danbooru.donmai.us/posts/")),
            post_list_url: Some(String::from("https://danbooru.donmai.us/posts.json")),
            pool_idx_url: Some(String::from("https://danbooru.donmai.us/pools")),
            max_post_limit: 200,
            auth_url: Some(String::from("https://danbooru.donmai.us/profile.json")),
            image_url: None,
        }
    }
}

impl Display for ServerConfig {
    /// Formats the `ServerConfig` for display, showing its `name`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
