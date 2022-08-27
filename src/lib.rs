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

// Export newer queue impl
pub use imageboards::queue::Queue;

pub use imageboards::rating::Rating;

pub use imageboards::post::Post;

pub use imageboards::extractors::ImageBoardExtractor;

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
macro_rules! bail_error {
    ($err:expr) => {{
        return Err($err.into());
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

#[macro_export]
macro_rules! finish_and_print_results {
    ($bars:expr, $self:expr, $auth_res:expr) => {{
        $bars.main.finish_and_clear();
        println!(
            "{} {} {}",
            $self
                .counters
                .downloaded_mtx
                .lock()
                .unwrap()
                .to_string()
                .bold()
                .blue(),
            "files".bold().blue(),
            "downloaded".bold()
        );

        if $auth_res.is_some() && $self.blacklisted_posts > 0 {
            println!(
                "{} {}",
                $self.blacklisted_posts.to_string().bold().red(),
                "posts with blacklisted tags were not downloaded."
                    .bold()
                    .red()
            )
        }
    }};
    ($bars:expr, $self:expr) => {{
        $bars.main.finish_and_clear();
        println!(
            "{} {} {}",
            $self
                .counters
                .downloaded_mtx
                .lock()
                .unwrap()
                .to_string()
                .bold()
                .blue(),
            "files".bold().blue(),
            "downloaded".bold()
        );
    }};
}
