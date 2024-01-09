use ibdl_common::serde;
use ibdl_common::{
    serde::{Deserialize, Serialize},
    ImageBoards,
};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fmt::Display;

pub const EXTRACTOR_UA_NAME: &str = "Rust Imageboard Post Extractor";
pub const CLIENT_UA_NAME: &str = "Rust Imageboard Downloader";

pub mod serialize;

pub static DEFAULT_SERVERS: Lazy<HashMap<String, ServerConfig>> = Lazy::new(|| {
    let mut hmap = HashMap::with_capacity(6);
    hmap.insert(
        "danbooru".to_string(),
        ServerConfig {
            name: String::from("danbooru"),
            pretty_name: String::from("Danbooru"),
            server: ImageBoards::Danbooru,
            client_user_agent: format!(
                "{}/{} (by danbooru user FerrahWolfeh)",
                CLIENT_UA_NAME,
                env!("CARGO_PKG_VERSION")
            ),
            extractor_user_agent: format!(
                "{}/{} (by danbooru user FerrahWolfeh)",
                EXTRACTOR_UA_NAME,
                env!("CARGO_PKG_VERSION")
            ),
            base_url: String::from("https://danbooru.donmai.us"),
            post_url: Some(String::from("https://danbooru.donmai.us/posts/")),
            post_list_url: Some(String::from("https://danbooru.donmai.us/posts.json")),
            pool_idx_url: Some(String::from("https://danbooru.donmai.us/pools")),
            max_post_limit: 200,
            auth_url: Some(String::from("https://danbooru.donmai.us/profile.json")),
        },
    );
    hmap.insert(
        "e621".to_string(),
        ServerConfig {
            name: String::from("e621"),
            server: ImageBoards::E621,
            pretty_name: String::from("e621"),
            client_user_agent: format!(
                "{}/{} (by e621 user FerrahWolfeh)",
                CLIENT_UA_NAME,
                env!("CARGO_PKG_VERSION")
            ),
            extractor_user_agent: format!(
                "{}/{} (by e621 user FerrahWolfeh)",
                EXTRACTOR_UA_NAME,
                env!("CARGO_PKG_VERSION")
            ),
            base_url: String::from("https://e621.net"),
            post_url: Some(String::from("https://e621.net/posts/")),
            post_list_url: Some(String::from("https://e621.net/posts.json")),
            pool_idx_url: Some(String::from("https://e621.net/pools")),
            max_post_limit: 320,
            auth_url: Some(String::from("https://e621.net/users/")),
        },
    );
    hmap.insert(
        "gelbooru".to_string(),
        ServerConfig {
            name: String::from("gelbooru"),
            pretty_name: String::from("Gelbooru"),
            server: ImageBoards::Gelbooru,
            client_user_agent: format!("{}/{}", CLIENT_UA_NAME, env!("CARGO_PKG_VERSION")),
            extractor_user_agent: format!("{}/{}", EXTRACTOR_UA_NAME, env!("CARGO_PKG_VERSION")),
            base_url: String::from("https://gelbooru.com"),
            post_url: Some(String::from(
                "http://gelbooru.com/index.php?page=dapi&s=post&q=index&json=1",
            )),
            post_list_url: Some(String::from(
                "http://gelbooru.com/index.php?page=dapi&s=post&q=index&json=1",
            )),
            pool_idx_url: None,
            max_post_limit: 100,
            auth_url: None,
        },
    );
    hmap.insert(
        "rule34".to_string(),
        ServerConfig {
            name: String::from("rule34"),
            pretty_name: String::from("Rule34"),
            server: ImageBoards::Rule34,
            client_user_agent: format!("{}/{}", CLIENT_UA_NAME, env!("CARGO_PKG_VERSION")),
            extractor_user_agent: format!("{}/{}", EXTRACTOR_UA_NAME, env!("CARGO_PKG_VERSION")),
            base_url: String::from("https://rule34.xxx"),
            post_url: Some(String::from(
                "https://api.rule34.xxx/index.php?page=dapi&s=post&q=index&json=1",
            )),
            post_list_url: Some(String::from(
                "https://api.rule34.xxx/index.php?page=dapi&s=post&q=index&json=1",
            )),
            pool_idx_url: None,
            max_post_limit: 1000,
            auth_url: None,
        },
    );
    hmap.insert(
        "realbooru".to_string(),
        ServerConfig {
            name: String::from("realbooru"),
            pretty_name: String::from("Realbooru"),
            server: ImageBoards::Gelbooru,
            client_user_agent: format!("{}/{}", CLIENT_UA_NAME, env!("CARGO_PKG_VERSION")),
            extractor_user_agent: format!("{}/{}", EXTRACTOR_UA_NAME, env!("CARGO_PKG_VERSION")),
            base_url: String::from("https://realbooru.com"),
            post_url: Some(String::from(
                "http://realbooru.com/index.php?page=dapi&s=post&q=index&json=1",
            )),
            post_list_url: Some(String::from(
                "http://realbooru.com/index.php?page=dapi&s=post&q=index&json=1",
            )),
            pool_idx_url: None,
            max_post_limit: 1000,
            auth_url: None,
        },
    );
    hmap.insert(
        "konachan".to_string(),
        ServerConfig {
            name: String::from("konachan"),
            pretty_name: String::from("Konachan"),
            server: ImageBoards::Konachan,
            client_user_agent: format!("{}/{}", CLIENT_UA_NAME, env!("CARGO_PKG_VERSION")),
            extractor_user_agent: format!("{}/{} ", EXTRACTOR_UA_NAME, env!("CARGO_PKG_VERSION")),
            base_url: String::from("https://konachan.com"),
            post_url: None,
            post_list_url: Some(String::from("https://konachan.com/post.json")),
            pool_idx_url: None,
            max_post_limit: 100,
            auth_url: None,
        },
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
            client_user_agent: format!(
                "{}/{} (by danbooru user FerrahWolfeh)",
                CLIENT_UA_NAME,
                env!("CARGO_PKG_VERSION")
            ),
            extractor_user_agent: format!(
                "{}/{} (by danbooru user FerrahWolfeh)",
                EXTRACTOR_UA_NAME,
                env!("CARGO_PKG_VERSION")
            ),
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
