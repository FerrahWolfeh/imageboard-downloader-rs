use std::{
    env,
    fs::create_dir_all,
    io,
    path::{Path, PathBuf},
};

// Public Exports
pub use bincode;
pub use directories;
pub use log;
pub use reqwest;
pub use serde;
pub use serde_json;
pub use tokio;

use directories::ProjectDirs;

use log::debug;

use serde::{Deserialize, Serialize};

pub mod macros;
pub mod post;

/// All currently supported imageboards and their underlying attributes
#[derive(Debug, Copy, Clone, Ord, PartialOrd, PartialEq, Eq, Serialize, Deserialize)]
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
    #[inline]
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

    #[inline]
    pub fn extractor_user_agent(self) -> String {
        let app_name = "Rust Imageboard Post Extractor";
        let variant = match self {
            ImageBoards::Danbooru => " (by danbooru user FerrahWolfeh)",
            ImageBoards::E621 => " (by e621 user FerrahWolfeh)",
            _ => "",
        };
        let ua = format!("{}/{}{}", app_name, env!("CARGO_PKG_VERSION"), variant);
        debug!("Using user-agent: {}", ua);
        ua
    }

    /// Returns the base URL for the website.
    #[inline]
    pub fn base_url(&self) -> &'static str {
        match self {
            ImageBoards::Danbooru => "https://danbooru.donmai.us",
            ImageBoards::E621 => "https://e621.net",
            ImageBoards::Rule34 => "https://rule34.xxx",
            ImageBoards::Konachan => "https://konachan.com",
            ImageBoards::Realbooru => "https://realbooru.com",
            ImageBoards::Gelbooru => "https://gelbooru.com",
        }
    }

    /// Returns the endpoint for the post list with their respective tags.
    #[inline]
    pub fn post_url(&self) -> &'static str {
        match self {
            ImageBoards::Danbooru => "https://danbooru.donmai.us/posts/",
            ImageBoards::E621 => "https://e621.net/posts/",
            ImageBoards::Rule34 => {
                "https://api.rule34.xxx/index.php?page=dapi&s=post&q=index&json=1"
            }
            ImageBoards::Konachan => "",
            ImageBoards::Realbooru => {
                "http://realbooru.com/index.php?page=dapi&s=post&q=index&json=1"
            }
            ImageBoards::Gelbooru => {
                "http://gelbooru.com/index.php?page=dapi&s=post&q=index&json=1"
            }
        }
    }

    /// Returns the endpoint for the post list with their respective tags.
    #[inline]
    pub fn post_list_url(&self) -> &'static str {
        match self {
            ImageBoards::Danbooru => "https://danbooru.donmai.us/posts.json",
            ImageBoards::E621 => "https://e621.net/posts.json",
            ImageBoards::Rule34 => {
                "https://api.rule34.xxx/index.php?page=dapi&s=post&q=index&json=1"
            }
            ImageBoards::Konachan => "https://konachan.com/post.json",
            ImageBoards::Realbooru => {
                "http://realbooru.com/index.php?page=dapi&s=post&q=index&json=1"
            }
            ImageBoards::Gelbooru => {
                "http://gelbooru.com/index.php?page=dapi&s=post&q=index&json=1"
            }
        }
    }

    #[inline]
    pub fn pool_idx_url(self) -> &'static str {
        match self {
            ImageBoards::Danbooru => "https://danbooru.donmai.us/pools",
            ImageBoards::E621 => "https://e621.net/pools",
            _ => "",
        }
    }

    /// Returns max number of posts per page a imageboard can have
    #[inline]
    pub fn max_post_limit(self) -> usize {
        match self {
            ImageBoards::Danbooru => 200,
            ImageBoards::E621 => 320,
            ImageBoards::Rule34 | ImageBoards::Realbooru => 1000,
            ImageBoards::Konachan | ImageBoards::Gelbooru => 100,
        }
    }

    /// Returns the url used for validating the login input and parsing the user's profile.
    #[inline]
    pub fn auth_url(self) -> &'static str {
        match self {
            ImageBoards::Danbooru => "https://danbooru.donmai.us/profile.json",
            ImageBoards::E621 => "https://e621.net/users/",
            _ => "",
        }
    }

    /// Returns a `PathBuf` pointing to the imageboard's authentication cache.
    ///
    /// This is XDG-compliant and saves cache files to
    /// `$XDG_CONFIG_HOME/imageboard-downloader/<imageboard>` on Linux or
    /// `%APPDATA%/FerrahWolfeh/imageboard-downloader/<imageboard>` on Windows
    ///
    /// Or you can set the env var `IBDL_CACHE_DIR` to point it to a custom location.
    #[inline]
    pub fn auth_cache_dir() -> Result<PathBuf, io::Error> {
        let cfg_path = env::var("IBDL_CACHE_DIR").unwrap_or({
            let cdir = ProjectDirs::from("com", "FerrahWolfeh", "imageboard-downloader").unwrap();
            cdir.config_dir().to_string_lossy().to_string()
        });

        let cfold = Path::new(&cfg_path);

        if !cfold.exists() {
            create_dir_all(cfold)?;
        }

        Ok(cfold.to_path_buf())
    }
}
