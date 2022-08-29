//! # Imageboard Downloader
//!
//! imageboard_downloader is a CLI utility to bulk download images from popular imageboard (booru)
//! websites.
//!
//! This utility aims to be fast, portable and lightweigth while offering simultaneous downloads
//! and more.
//mod config;
pub mod imageboards;
mod progress_bars;

// Export main representative enum
pub use imageboards::ImageBoards;

// Export newer queue impl
pub use imageboards::queue::Queue;

pub use imageboards::post::Post;

pub use imageboards::extractors::Auth;
pub use imageboards::extractors::Extractor;

pub use imageboards::extractors::danbooru::DanbooruExtractor;
pub use imageboards::extractors::e621::E621Extractor;
pub use imageboards::extractors::gelbooru::GelbooruExtractor;
pub use imageboards::extractors::moebooru::MoebooruExtractor;

#[macro_export]
macro_rules! client {
    ($x:expr) => {{
        Client::builder().user_agent($x).build().unwrap()
    }};
}

#[macro_export]
macro_rules! join_tags {
    ($x:expr) => {{
        let tl = $x.join("+");
        debug!("Tag List: {}", tl);
        tl
    }};
}

#[macro_export]
macro_rules! extract_ext_from_url {
    ($x:expr) => {{
        let ext = $x.split('.').next_back().unwrap();
        ext.to_string()
    }};
}

#[macro_export]
macro_rules! print_found {
    ($vec:expr) => {{
        print!(
            "\r{} {} {}",
            "Found".bold(),
            $vec.len().to_string().bold().blue(),
            "posts".bold()
        );
        io::stdout().flush().unwrap();
    }};
}
