use std::{
    env,
    fs::create_dir_all,
    io,
    path::{Path, PathBuf},
    str::FromStr,
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
    /// Represents the website ```http://realbooru.com```
    GelbooruV0_2,
    /// Represents the website ```https://konachan.com``` or it's safe variant ```https://konachan.net```.
    Moebooru,
    /// Represents the website ```https://gelbooru.com```.
    Gelbooru,
}

impl ToString for ImageBoards {
    fn to_string(&self) -> String {
        match self {
            ImageBoards::Danbooru => String::from("Danbooru"),
            ImageBoards::E621 => String::from("e621"),
            ImageBoards::GelbooruV0_2 => String::from("Gelbooru Beta V0.2.0"),
            ImageBoards::Moebooru => String::from("Moebooru"),
            ImageBoards::Gelbooru => String::from("Gelbooru"),
        }
    }
}

impl FromStr for ImageBoards {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "gelbooru" => Ok(Self::Gelbooru),
            "gelbooru_020" | "gelbooru beta 0.2" | "realbooru" => Ok(Self::GelbooruV0_2),
            "danbooru" => Ok(Self::Danbooru),
            "e621" => Ok(Self::E621),
            "moebooru" => Ok(Self::Moebooru),
            _ => Err(String::from("Invalid imageboard type.")),
        }
    }
}

impl ImageBoards {
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
