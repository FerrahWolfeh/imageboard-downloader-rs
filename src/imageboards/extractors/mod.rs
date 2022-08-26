//! Modules that work by parsing API info from imageboards and generating `Post`s
//! # Extractors
//!
//! All modules inside work by connecting to a imageboard website, search for posts with the tags supplied and parse all of them into a `Vec<Post>`

use num_traits::Unsigned;

use crate::Post;

use self::error::ExtractorError;

#[cfg(feature = "danbooru")]
pub mod danbooru;

#[cfg(feature = "e621")]
pub mod e621;

#[cfg(feature = "gelbooru")]
pub mod gelbooru;

mod error;
#[cfg(feature = "moebooru")]
pub mod moebooru;

/// This trait is the only public interface all the extractors should expose
pub trait ImageBoardExtractor {
    /// Searches the tags list on a per-page way. It's relatively the fastest way, but subject to slowdowns since it needs
    /// to iter through all pages manually in order to fetch all posts.
    fn search<T, I>(&self, tags: &[T], page: I) -> Result<Vec<Post>, ExtractorError>
    where
        T: Into<String>,
        I: Unsigned;

    /// Searches all posts from all pages with given tags, it's the most pratical one, but slower on startup since it will search all pages by itself until it finds no more posts.
    fn full_search<T>(&self, tags: &[T]) -> Result<Vec<Post>, ExtractorError>
    where
        T: Into<String>;
}
