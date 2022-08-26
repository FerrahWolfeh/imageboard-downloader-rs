//! # Imageboard Downloader
//!
//! imageboard_downloader is a CLI utility to bulk download images from popular imageboard (booru)
//! websites.
//!
//! This utility aims to be fast, portable and lightweigth while offering simultaneous downloads
//! and more.
pub mod imageboards;
mod progress_bars;

// Export main representative enum
pub use imageboards::ImageBoards;

// Export main worker queue
pub use imageboards::queue::DownloadQueue;

pub use imageboards::rating::Rating;

pub use imageboards::post::Post;

#[cfg(feature = "global_blacklist")]
pub use imageboards::blacklist::GlobalBlacklist;

#[cfg(feature = "danbooru")]
pub use imageboards::extractors::danbooru::DanbooruDownloader;
#[cfg(feature = "e621")]
pub use imageboards::extractors::e621::E621Downloader;
#[cfg(feature = "gelbooru")]
pub use imageboards::extractors::gelbooru::GelbooruDownloader;
#[cfg(feature = "moebooru")]
pub use imageboards::extractors::moebooru::MoebooruDownloader;
