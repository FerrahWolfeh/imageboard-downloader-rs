use ibdl_common::serde;
use ibdl_common::{
    serde::{Deserialize, Serialize},
    ImageBoards,
};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fmt::Display;

use crate::imageboards::{prelude::*, Extractor, ExtractorFeatures};
use crate::server_config;

pub(crate) const DEFAULT_EXT_UA: &str =
    concat!("Rust Imageboard Post Extractor/", env!("CARGO_PKG_VERSION"));

pub(crate) const DEFAULT_CLI_UA: &str =
    concat!("Rust Imageboard Downloader/", env!("CARGO_PKG_VERSION"));

pub(crate) const DB_EXT_UA: &str = concat!(
    "Rust Imageboard Post Extractor/",
    env!("CARGO_PKG_VERSION"),
    " (by Danbooru user FerrahWolfeh)"
);

pub(crate) const DB_CLI_UA: &str = concat!(
    "Rust Imageboard Downloader/",
    env!("CARGO_PKG_VERSION"),
    " (by Danbooru user FerrahWolfeh)"
);

pub(crate) const E621_CLI_UA: &str = concat!(
    "Rust Imageboard Downloader/",
    env!("CARGO_PKG_VERSION"),
    " (by e621 user FerrahWolfeh)"
);

pub(crate) const E621_EXT_UA: &str = concat!(
    "Rust Imageboard Post Extractor/",
    env!("CARGO_PKG_VERSION"),
    " (by e621 user FerrahWolfeh)"
);

pub mod macros;
pub mod serialize;

pub static DEFAULT_SERVERS: Lazy<HashMap<String, ServerConfig>> = Lazy::new(|| {
    let mut hmap = HashMap::with_capacity(6);
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
            Some(String::from("https://danbooru.donmai.us/profile.json"))
        ),
    );
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
            Some(String::from("https://e621.net/users/"))
        ),
    );
    hmap.insert(
        "gelbooru".to_string(),
        server_config!(
            "gelbooru",
            "Gelbooru",
            ImageBoards::Gelbooru,
            DEFAULT_CLI_UA,
            DEFAULT_EXT_UA,
            "https://gelbooru.com",
            Some(String::from(
                "http://gelbooru.com/index.php?page=dapi&s=post&q=index&json=1"
            )),
                "http://gelbooru.com/index.php?page=dapi&s=post&q=index&json=1",
            None,
            100,
            None
        ),
    );
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
            None
        ),
    );
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
            None
        ),
    );
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
    pub max_post_limit: usize,
    pub auth_url: Option<String>,
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
        }
    }
}

impl Display for ServerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
