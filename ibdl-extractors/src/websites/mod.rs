//! Modules that work by parsing post info from a imageboard API into a list of [Posts](crate::imageboards::post).
//! # Extractors
//!
//! All modules implementing [`Extractor`] work by connecting to a imageboard website, searching for posts with the tags supplied and parsing all of them into a [`PostQueue`](crate::imageboards::post::PostQueue).
//!
//! ## General example
//! Most extractors have a common set of public methods that should be the default way of implementing and interacting with them.
//!
//! ### Example with the `Danbooru` extractor
//! ```rust
//! use imageboard_downloader::*;
//!
//! async fn test() {
//!     let tags = ["umbreon", "espeon"]; // The tags to search
//!     
//!     let safe_mode = false; // Setting this to true, will ignore searching NSFW posts
//!
//!     let disable_blacklist = false; // Will filter all items according to what's set in GBL
//!
//!     let mut unit = DanbooruExtractor::new(&tags, safe_mode, disable_blacklist); // Initialize
//!
//!     let prompt = true; // If true, will ask the user to input thei username and API key.
//!
//!     unit.auth(prompt).await.unwrap(); // Try to authenticate
//!
//!     let start_page = Some(1); // Start searching from the first page
//!
//!     let limit = Some(50); // Max number of posts to download
//!
//!     let posts = unit.full_search(start_page, limit).await.unwrap(); // and then, finally search
//!
//!     println!("{:#?}", posts.posts);
//! }
//!```
//!
//! ### Example with the `Gelbooru` extractor
//!
//! The Gelbooru extractor supports multiple websites, so to use it correctly, an additional option needs to be set.
//!
//! ```rust
//! use imageboard_downloader::*;
//!
//! async fn test() {
//!     let tags = ["umbreon", "espeon"]; // The tags to search
//!     
//!     let safe_mode = false; // Setting this to true, will ignore searching NSFW posts
//!
//!     let disable_blacklist = false; // Will filter all items according to what's set in GBL
//!
//!     let mut unit = GelbooruExtractor::new(&tags, safe_mode, disable_blacklist)
//!         .set_imageboard(ImageBoards::Rule34).expect("Invalid imageboard"); // Here the imageboard was set to Rule34
//!
//!
//!     let prompt = true; // If true, will ask the user to input thei username and API key.
//!
//!     let start_page = Some(1); // Start searching from the first page
//!
//!     let limit = Some(50); // Max number of posts to download
//!
//!     let posts = unit.full_search(start_page, limit).await.unwrap(); // and then, finally search
//!
//!     println!("{:#?}", posts.posts);
//! }
//!```
//!
//!
use std::fmt::Display;

use async_trait::async_trait;
use ibdl_common::{
    post::{rating::Rating, Post, PostQueue},
    reqwest::Client,
    ImageBoards,
};

use crate::error::ExtractorError;

pub mod danbooru;

pub mod e621;

pub mod gelbooru;

pub mod moebooru;

/// This trait should be the only common public interface all extractors should expose aside from some other website-specific configuration.
#[async_trait]
pub trait Extractor {
    /// Sets up the extractor unit with the tags supplied.
    ///
    /// Will ignore `safe_mode` state if the imageboard doesn't have a safe variant.
    fn new<S>(tags: &[S], download_ratings: &[Rating], disable_blacklist: bool) -> Self
    where
        S: ToString + Display;

    /// Searches the tags list on a per-page way. It's relatively the fastest way, but subject to slowdowns since it needs
    /// to iter through all pages manually in order to fetch all posts.
    async fn search(&mut self, page: u16) -> Result<PostQueue, ExtractorError>;

    /// Searches all posts from all pages with given tags, it's the most pratical one, but slower on startup since it will search all pages by itself until it finds no more posts.
    async fn full_search(
        &mut self,
        start_page: Option<u16>,
        limit: Option<u16>,
    ) -> Result<PostQueue, ExtractorError>;

    /// Pretty similar to `search`, but instead returns the raw post list instead of a [`PostQueue`](ibdl_common::post::PostQueue)
    async fn get_post_list(&self, page: u16) -> Result<Vec<Post>, ExtractorError>;

    /// This is a separate lower level function to map posts by feeding a custom JSON object obtained through other means.
    fn map_posts(&self, raw_json: String) -> Result<Vec<Post>, ExtractorError>;

    /// Consumes `self` and returns the used client for external use.
    fn client(self) -> Client;

    /// Get the total number of removed files by the internal blacklist.
    fn total_removed(&self) -> u64;
}

/// Authentication capability for imageboard websites. Implies the Extractor is able to use a user-defined blacklist
#[async_trait]
pub trait Auth {
    /// Setting to `true` will prompt the user for username and API key, while setting to `false` will silently try to authenticate.
    ///
    /// Does nothing if auth was never configured.
    async fn auth(&mut self, prompt: bool) -> Result<(), ExtractorError>;
}

/// Indicates that the extractor is capable of extracting from multiple websites that share a similar API
pub trait MultiWebsite {
    /// Changes the state of the internal active imageboard. If not set, the extractor should default to something, but never `panic`.
    fn set_imageboard(self, imageboard: ImageBoards) -> Result<Self, ExtractorError>
    where
        Self: std::marker::Sized;
}
