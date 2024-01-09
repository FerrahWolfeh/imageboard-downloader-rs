use ahash::{HashMap, HashMapExt};
use ibdl_common::serde;
use ibdl_common::{
    serde::{Deserialize, Serialize},
    ImageBoards,
};
use once_cell::sync::Lazy;

pub const EXTRACTOR_UA_NAME: &str = "Rust Imageboard Post Extractor";
pub const CLIENT_UA_NAME: &str = "Rust Imageboard Downloader";

pub static DEFAULT_SERVERS: Lazy<HashMap<&str, ServerConfig>> = Lazy::new(|| {
    let mut hmap = HashMap::with_capacity(6);
    hmap.insert(
        "danbooru",
        ServerConfig {
            name: String::from("Danbooru"),
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
            post_url: String::from("https://danbooru.donmai.us/posts/"),
            post_list_url: String::from("https://danbooru.donmai.us/posts.json"),
            pool_idx_url: String::from("https://danbooru.donmai.us/pools"),
            max_post_limit: 200,
            auth_url: String::from("https://danbooru.donmai.us/profile.json"),
        },
    );
    hmap.insert(
        "e621",
        ServerConfig {
            server: ImageBoards::E621,
            name: String::from("e621"),
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
            post_url: String::from("https://e621.net/posts/"),
            post_list_url: String::from("https://e621.net/posts.json"),
            pool_idx_url: String::from("https://e621.net/pools"),
            max_post_limit: 320,
            auth_url: String::from("https://e621.net/users/"),
        },
    );
    hmap.insert(
        "gelbooru",
        ServerConfig {
            name: String::from("Gelbooru"),
            server: ImageBoards::Gelbooru,
            client_user_agent: format!("{}/{}", CLIENT_UA_NAME, env!("CARGO_PKG_VERSION")),
            extractor_user_agent: format!("{}/{}", EXTRACTOR_UA_NAME, env!("CARGO_PKG_VERSION")),
            base_url: String::from("https://gelbooru.com"),
            post_url: String::from("http://gelbooru.com/index.php?page=dapi&s=post&q=index&json=1"),
            post_list_url: String::from(
                "http://gelbooru.com/index.php?page=dapi&s=post&q=index&json=1",
            ),
            pool_idx_url: String::from(""),
            max_post_limit: 100,
            auth_url: String::from(""),
        },
    );
    hmap.insert(
        "rule34",
        ServerConfig {
            name: String::from("Rule34"),
            server: ImageBoards::Rule34,
            client_user_agent: format!("{}/{}", CLIENT_UA_NAME, env!("CARGO_PKG_VERSION")),
            extractor_user_agent: format!("{}/{}", EXTRACTOR_UA_NAME, env!("CARGO_PKG_VERSION")),
            base_url: String::from("https://rule34.xxx"),
            post_url: String::from(
                "https://api.rule34.xxx/index.php?page=dapi&s=post&q=index&json=1",
            ),
            post_list_url: String::from(
                "https://api.rule34.xxx/index.php?page=dapi&s=post&q=index&json=1",
            ),
            pool_idx_url: String::from(""),
            max_post_limit: 1000,
            auth_url: String::from(""),
        },
    );
    hmap.insert(
        "realbooru",
        ServerConfig {
            name: String::from("Realbooru"),
            server: ImageBoards::Gelbooru,
            client_user_agent: format!("{}/{}", CLIENT_UA_NAME, env!("CARGO_PKG_VERSION")),
            extractor_user_agent: format!("{}/{}", EXTRACTOR_UA_NAME, env!("CARGO_PKG_VERSION")),
            base_url: String::from("https://realbooru.com"),
            post_url: String::from(
                "http://realbooru.com/index.php?page=dapi&s=post&q=index&json=1",
            ),
            post_list_url: String::from(
                "http://realbooru.com/index.php?page=dapi&s=post&q=index&json=1",
            ),
            pool_idx_url: String::from(""),
            max_post_limit: 1000,
            auth_url: String::from(""),
        },
    );
    hmap.insert(
        "konachan",
        ServerConfig {
            name: String::from("Konachan"),
            server: ImageBoards::Konachan,
            client_user_agent: format!("{}/{}", CLIENT_UA_NAME, env!("CARGO_PKG_VERSION")),
            extractor_user_agent: format!("{}/{} ", EXTRACTOR_UA_NAME, env!("CARGO_PKG_VERSION")),
            base_url: String::from("https://konachan.com"),
            post_url: String::from(""),
            post_list_url: String::from("https://konachan.com/post.json"),
            pool_idx_url: String::from(""),
            max_post_limit: 100,
            auth_url: String::from(""),
        },
    );
    hmap
});

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "self::serde")]
pub struct ServerConfig {
    pub name: String,
    pub server: ImageBoards,
    pub client_user_agent: String,
    pub extractor_user_agent: String,
    pub base_url: String,
    pub post_url: String,
    pub post_list_url: String,
    pub pool_idx_url: String,
    pub max_post_limit: usize,
    pub auth_url: String,
}
