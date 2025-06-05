//! # IBDL-COMMON
//!
//! This crate is the common backbone for the `imageboard-downloader` project.
//! It provides shared data structures, utility functions, and re-exports common
//! dependencies used across the various parts of the imageboard downloader,
//! such as the core library and any potential frontends or tools.
//!
#![deny(clippy::all)]
use std::fmt::{Display, Formatter};
use std::{
    env,
    fs::create_dir_all,
    io,
    path::{Path, PathBuf},
    str::FromStr,
};

/// Re-export of the `directories` crate for platform-agnostic directory management.
pub use directories;

use directories::ProjectDirs;

use serde::{Deserialize, Serialize};

/// Contains utility macros used throughout the `imageboard-downloader` ecosystem.
pub mod macros;
/// Defines the `Post` structure and related functionalities for representing imageboard posts.
pub mod post;

/// Represents all currently supported imageboard websites.
///
/// This enum is central to identifying and configuring interactions with different
/// imageboard sources. It includes methods for obtaining domain names and handling
/// website-specific naming conventions.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageBoards {
    /// Represents the website ```https://danbooru.donmai.us``` or it's safe variant ```https://safebooru.donmai.us```.
    Danbooru,
    /// Represents the website ```https://e621.net``` or it's safe variant ```https://e926.net```.
    E621,
    /// Represents the website ```http://realbooru.com```
    #[serde(rename = "gelbooru_0.2")] // Keep serde name for compatibility if needed
    GelbooruV0_2,
    /// Represents the website ```https://konachan.com``` or it's safe variant ```https://konachan.net```.
    Moebooru,
    /// Represents the website ```https://gelbooru.com```
    Gelbooru,
}

impl ImageBoards {
    /// Returns the primary domain name for the imageboard.
    pub fn domain(self) -> &'static str {
        match self {
            Self::Danbooru => "https://danbooru.donmai.us",
            Self::E621 => "https://e621.net",
            Self::GelbooruV0_2 => "http://realbooru.com",
            Self::Moebooru => "https://konachan.com",
            Self::Gelbooru => "https://gelbooru.com",
        }
    }
}

impl Display for ImageBoards {
    /// Formats the `ImageBoards` variant into a user-friendly string.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Danbooru => write!(f, "Danbooru"),
            Self::E621 => write!(f, "e621"),
            Self::GelbooruV0_2 => write!(f, "Gelbooru Beta V0.2.0"),
            Self::Moebooru => write!(f, "Moebooru"),
            Self::Gelbooru => write!(f, "Gelbooru"),
        }
    }
}

impl FromStr for ImageBoards {
    /// Defines the possible error type when parsing a string into `ImageBoards`.
    type Err = String;

    /// Parses a string slice into an `ImageBoards` variant.
    /// This allows for flexible input, accepting common aliases for the imageboards.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "gelbooru" => Ok(Self::Gelbooru),
            "gelbooru_020" | "gelbooru beta 0.2" | "realbooru" => Ok(Self::GelbooruV0_2),
            "danbooru" => Ok(Self::Danbooru),
            "e621" => Ok(Self::E621),
            "moebooru" => Ok(Self::Moebooru),
            _ => Err(format!("Invalid or unsupported imageboard type: {}", s)),
        }
    }
}

impl ImageBoards {
    /// Returns a `PathBuf` pointing to the imageboard's authentication cache.
    ///
    /// The cache directory is determined in the following order:
    /// 1. If the `IBDL_CACHE_DIR` environment variable is set, its value is used.
    /// 2. Otherwise, a platform-specific XDG-compliant directory is used:
    ///    - Linux: `$XDG_CONFIG_HOME/imageboard-downloader/`
    ///    - Windows: `%APPDATA%/FerrahWolfeh/imageboard-downloader/`
    ///
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
    // Note: The auth_cache_dir is currently global and not per-imageboard.
}
