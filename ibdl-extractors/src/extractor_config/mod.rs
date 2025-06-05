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
use ibdl_common::serde;
use ibdl_common::{
    serde::{Deserialize, Serialize},
    ImageBoards,
};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fmt::Display;

pub(crate) const DEFAULT_EXT_UA: &str =
    concat!("Rust Imageboard Post Extractor/", env!("CARGO_PKG_VERSION"));

pub(crate) const DEFAULT_CLI_UA: &str =
    concat!("Rust Imageboard Downloader/", env!("CARGO_PKG_VERSION"));

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

pub mod macros;
pub mod serialize;

/// Static map of all available server configs
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "self::serde")]
pub struct ServerConfig {
    pub name: String,
    pub pretty_name: String,
    pub server: ImageBoards,
    pub client_user_agent: String,
    pub extractor_user_agent: String,
    pub base_url: String,
    pub post_url: Option<String>,
    pub post_list_url: Option<String>,
    pub pool_idx_url: Option<String>,
    pub max_post_limit: u16,
    pub auth_url: Option<String>,
    pub image_url: Option<String>,
}

impl ServerConfig {
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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
