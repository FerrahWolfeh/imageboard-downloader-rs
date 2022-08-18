pub mod imageboards;
mod progress_bars;

// Export main representative enum
pub use imageboards::ImageBoards;

// Export all downloader interfaces
pub use imageboards::danbooru::DanbooruDownloader;
pub use imageboards::e621::E621Downloader;
pub use imageboards::konachan::KonachanDownloader;
pub use imageboards::realbooru::RealbooruDownloader;
pub use imageboards::rule34::R34Downloader;
