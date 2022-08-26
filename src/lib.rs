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


pub use imageboards::blacklist::GlobalBlacklist;

// Export all downloader interfaces
pub use imageboards::danbooru::DanbooruDownloader;
pub use imageboards::e621::E621Downloader;
pub use imageboards::gelbooru::GelbooruDownloader;
pub use imageboards::moebooru::MoebooruDownloader;
