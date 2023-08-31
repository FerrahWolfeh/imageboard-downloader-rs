//! Modules that work by parsing post info from a imageboard API into a list of [Posts](ibdl_common::post).
//! # Extractors
//!
//! All modules implementing [`Extractor`] work by connecting to a imageboard website, searching for posts with the tags supplied and parsing all of them into a [`PostQueue`](ibdl_common::post::PostQueue).
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
use std::{
    fmt::Display,
    sync::{atomic::AtomicU64, Arc},
};

use crate::auth::ImageboardConfig;
use async_trait::async_trait;
use ibdl_common::{
    post::{extension::Extension, rating::Rating, Post, PostQueue},
    reqwest::Client,
    tokio::{sync::mpsc::UnboundedSender, task::JoinHandle},
    ImageBoards,
};

use crate::error::ExtractorError;

pub mod danbooru;

pub mod e621;

pub mod gelbooru;

pub mod moebooru;

pub type ExtractorThreadHandle = JoinHandle<Result<u64, ExtractorError>>;

/// This trait should be the only common public interface all extractors should expose aside from some other website-specific configuration.
#[async_trait]
pub trait Extractor {
    /// Sets up the extractor unit with the tags supplied.
    ///
    /// Will ignore `safe_mode` state if the imageboard doesn't have a safe variant.
    fn new<S>(
        tags: &[S],
        download_ratings: &[Rating],
        disable_blacklist: bool,
        map_videos: bool,
    ) -> Self
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

    /// Adds additional tags to the [blacklist filter](ibdl_extractors::blacklist::BlacklistFilter)
    fn exclude_tags(&mut self, tags: &[String]) -> &mut Self;

    /// Forces the extractor to only map posts that have the specified extension
    fn force_extension(&mut self, extension: Extension) -> &mut Self;

    /// Pretty similar to `search`, but instead returns the raw post list instead of a [`PostQueue`](ibdl_common::post::PostQueue)
    async fn get_post_list(&self, page: u16) -> Result<Vec<Post>, ExtractorError>;

    /// This is a separate lower level function to map posts by feeding a custom JSON object obtained through other means.
    fn map_posts(&self, raw_json: String) -> Result<Vec<Post>, ExtractorError>;

    /// Returns the used client for external use.
    fn client(&self) -> Client;

    /// Get the total number of removed files by the internal blacklist.
    fn total_removed(&self) -> u64;

    /// Returns the [`ImageBoards`](ibdl_common::ImageBoards) variant for this extractor
    fn imageboard(&self) -> ImageBoards;
}

/// Authentication capability for imageboard websites. Implies the Extractor is able to use a user-defined blacklist
#[async_trait]
pub trait Auth {
    /// Authenticates to the imageboard using the supplied [`Config`](crate::auth::ImageboardConfig)
    async fn auth(&mut self, config: ImageboardConfig) -> Result<(), ExtractorError>;
}

/// Indicates that the extractor is capable of extracting from multiple websites that share a similar API
pub trait MultiWebsite {
    /// Changes the state of the internal active imageboard. If not set, the extractor should default to something, but never `panic`.
    fn set_imageboard(&mut self, imageboard: ImageBoards) -> &mut Self
    where
        Self: std::marker::Sized + Extractor;
}

/// Capability for the extractor asynchronously send posts through a [`unbounded_channel`](ibdl_common::tokio::sync::mpsc::unbounded_channel) to another thread.
#[async_trait]
pub trait AsyncFetch {
    /// Simliar to [`full_search`](Extractor::full_search) in functionality, but instead of returning a [`PostQueue`](ibdl_common::post::PostQueue), sends posts asynchronously through a channel.
    async fn async_fetch(
        &mut self,
        sender_channel: UnboundedSender<Post>,
        start_page: Option<u16>,
        limit: Option<u16>,
        post_counter: Option<Arc<AtomicU64>>,
    ) -> Result<u64, ExtractorError>;

    /// High-level convenience thread builder for [`async_fetch`](crate::websites::AsyncFetch::async_fetch)
    fn setup_fetch_thread(
        self,
        sender_channel: UnboundedSender<Post>,
        start_page: Option<u16>,
        limit: Option<u16>,
        post_counter: Option<Arc<AtomicU64>>,
    ) -> JoinHandle<Result<u64, ExtractorError>>;
}
