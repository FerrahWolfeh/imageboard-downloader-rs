//! Modules that work by parsing API info from imageboards and generating `Post`s
//! # Extractors
//!
//! All modules inside work by connecting to a imageboard website, search for posts with the tags supplied and parse all of them into a `Vec<Post>`
use self::error::ExtractorError;
use super::queue::PostQueue;
use async_trait::async_trait;

pub mod danbooru;

pub mod e621;

pub mod gelbooru;

mod error;
pub mod moebooru;

/// This trait is the only public interface all the extractors should expose aside from some other website-specific configuration.
#[async_trait]
pub trait ImageBoardExtractor {
    /// Sets up the extractor unit with the tags supplied. Will ignore `safe_mode` state if the imageboard doesn't have a safe variant.
    fn new(tags: &[String], safe_mode: bool) -> Self;
    /// Searches the tags list on a per-page way. It's relatively the fastest way, but subject to slowdowns since it needs
    /// to iter through all pages manually in order to fetch all posts.
    async fn search(&mut self, page: usize) -> Result<PostQueue, ExtractorError>;

    /// Searches all posts from all pages with given tags, it's the most pratical one, but slower on startup since it will search all pages by itself until it finds no more posts.
    async fn full_search(&mut self) -> Result<PostQueue, ExtractorError>;
}
