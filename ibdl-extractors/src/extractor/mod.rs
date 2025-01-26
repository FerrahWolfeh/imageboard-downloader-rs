//! Modules that work by parsing post info from a imageboard API into a list of [Posts](ibdl_common::post).
//! # Extractors
//!
//! All modules implementing [`Extractor`] work by connecting to a imageboard website, searching for posts with the tags supplied and parsing all of them into a [`PostQueue`](PostQueue).
//!
#![deny(clippy::nursery)]
use crate::extractor_config::ServerConfig;
use ibdl_common::{
    post::{extension::Extension, rating::Rating, Post, PostQueue},
    reqwest::Client,
    ImageBoards,
};
use std::{fmt::Display, future::Future};

use crate::error::ExtractorError;
use crate::extractor::caps::ExtractorFeatures;

pub mod caps;
pub mod common;

/// This trait should be the only common public interface all extractors should expose aside from some other website-specific configuration.
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

    /// Sets up the extractor unit with the tags supplied.
    ///
    /// Will ignore `safe_mode` state if the imageboard doesn't have a safe variant.
    fn new_with_config<S>(
        tags: &[S],
        download_ratings: &[Rating],
        disable_blacklist: bool,
        map_videos: bool,
        config: ServerConfig,
    ) -> Self
    where
        S: ToString + Display;

    /// Searches the tags list on a per-page way. It's relatively the fastest way, but subject to slowdowns since it needs
    /// to iter through all pages manually in order to fetch all posts.
    fn search(
        &mut self,
        page: u16,
    ) -> impl Future<Output = Result<PostQueue, ExtractorError>> + Send;

    /// Searches all posts from all pages with given tags, it's the most practical one, but slower on startup since it will search all pages by itself until it finds no more posts.
    fn full_search(
        &mut self,
        start_page: Option<u16>,
        limit: Option<u16>,
    ) -> impl Future<Output = Result<PostQueue, ExtractorError>> + Send;

    /// Adds additional tags to the [blacklist filter](ibdl_extractors::blacklist::BlacklistFilter)
    fn exclude_tags(&mut self, tags: &[String]) -> &mut Self;

    /// Forces the extractor to only map posts that have the specified extension
    fn force_extension(&mut self, extension: Extension) -> &mut Self;

    /// Pretty similar to `search`, but instead returns the raw post list instead of a [`PostQueue`](ibdl_common::post::PostQueue)
    fn get_post_list(
        &self,
        page: u16,
        limit: Option<u16>,
    ) -> impl Future<Output = Result<Vec<Post>, ExtractorError>> + Send;

    /// This is a separate lower level function to map posts by feeding a custom JSON object obtained through other means.
    fn map_posts(&self, raw_json: String) -> Result<Vec<Post>, ExtractorError>;

    /// Returns the used client for external use.
    fn client(&self) -> Client;

    /// Get the total number of removed files by the internal blacklist.
    fn total_removed(&self) -> u64;

    /// Returns the [`ImageBoards`](ibdl_common::ImageBoards) variant for this extractor
    fn imageboard(&self) -> ImageBoards;

    /// Expose some bitflags to indicate the features this extractor should support
    fn features() -> ExtractorFeatures;

    /// Return the current configured [server](crate::extractor_config) for this extractor
    fn config(&self) -> ServerConfig;
}
