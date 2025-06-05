//! # Post Module
//!
//! This module defines the core structures and utilities for representing
//! an imageboard post within the `imageboard-downloader` ecosystem.
//!
//! The main entity is the [`Post`](crate::post::Post) struct, which encapsulates common information
//! extracted from imageboard APIs, such as image URL, ID, tags, and rating.
//!
//! It also includes:
//! - [`PostQueue`](crate::post::PostQueue): A structure to hold a collection of posts fetched from an imageboard,
//!   along with associated metadata like the client used and search tags.
//! - [`NameType`](crate::post::NameType): An enum to specify whether to use a post's ID or MD5 hash for
//!   its filename.
//! - Submodules for handling specific aspects of a post:
//!   - [`error`](crate::post::error): Defines errors specific to post handling.
//!   - [`extension`](crate::post::extension): Manages file extensions.
//!   - [`rating`](crate::post::rating): Represents content safety ratings.
//!   - [`tags`](crate::post::tags): Handles post tags.
use reqwest::Client;
use serde::{Deserialize, Serialize};

use std::{cmp::Ordering, fmt::Debug, ops::Not};

use crate::{
    post::{extension::Extension, rating::Rating, tags::Tag},
    ImageBoards,
};

/// Defines errors that can occur during post processing or operations.
pub mod error;
/// Handles the file extension of a post.
pub mod extension;
/// Represents the safety rating of a post (e.g., Safe, Questionable, Explicit).
pub mod rating;
/// Manages the tags associated with a post.
pub mod tags;

/// Specifies the naming convention for downloaded files.
///
/// This enum allows users to choose whether the downloaded file should be named
/// using the post's unique ID or its MD5 hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NameType {
    ID,
    MD5,
}

impl Not for NameType {
    /// The resulting `NameType` after inversion.
    type Output = Self;

    /// Inverts the `NameType`.
    /// `ID` becomes `MD5`, and `MD5` becomes `ID`.
    fn not(self) -> Self::Output {
        match self {
            Self::ID => Self::MD5,
            Self::MD5 => Self::ID,
        }
    }
}

/// Represents a collection of posts fetched from an imageboard.
///
/// This structure holds the posts themselves, along with context such as the
/// imageboard source, the client used for fetching, and the tags used for the search.
#[derive(Debug)]
pub struct PostQueue {
    /// The imageboard where the posts come from.
    pub imageboard: ImageBoards,
    /// The internal `Client` used by the extractor.
    pub client: Client,
    /// A list containing all `Post`s collected.
    pub posts: Vec<Post>,
    /// The tags used to search the collected posts.
    pub tags: Vec<String>,
}

impl PostQueue {
    /// Prepares the post queue, potentially limiting the number of posts.
    ///
    /// If `limit` is `Some`, the `posts` vector will be truncated to that size.
    /// If `limit` is `None`, the `posts` vector will be shrunk to fit its current
    /// content, potentially freeing up unused capacity.
    pub fn prepare(&mut self, limit: Option<u16>) {
        if let Some(max) = limit {
            self.posts.truncate(max as usize);
        } else {
            self.posts.shrink_to_fit()
        }
    }
}

/// Represents a single imageboard post.
///
/// This struct consolidates the essential information extracted from an imageboard's
/// API, necessary for identifying, downloading, and saving the associated media file.
#[derive(Clone, Serialize, Deserialize, Eq)]
pub struct Post {
    /// ID number of the post given by the imageboard
    pub id: u64,
    /// The imageboard where this post was extracted from
    pub website: ImageBoards,
    /// Direct URL of the original image file located inside the imageboard's server
    pub url: String,
    /// Instead of calculating the downloaded file's MD5 hash on the fly, it uses the one provided by the API.
    pub md5: String,
    /// The original file extension provided by the imageboard.
    ///
    /// ```https://konachan.com``` (Moebooru) and some other imageboards don't provide this field. So, additional work is required to get the file extension from the url
    pub extension: Extension,
    /// Rating of the post. Can be:
    ///
    /// * `Rating::Safe` for SFW posts
    /// * `Rating::Questionable` for a not necessarily SFW post
    /// * `Rating::Explicit` for NSFW posts
    /// * `Rating::Unknown` in case none of the above are correctly parsed
    pub rating: Rating,
    /// Set of tags associated with the post.
    ///
    /// Used to exclude posts according to a blacklist
    pub tags: Vec<Tag>,
}

impl Debug for Post {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Post")
            .field("Post ID", &self.id)
            .field("Website", &self.website)
            .field("Download URL", &self.url)
            .field("MD5 Hash", &self.md5)
            .field("File Extension", &self.extension)
            .field("Rating", &self.rating)
            .field("Tag List", &self.tags)
            .finish()
    }
}

impl Ord for Post {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for Post {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Post {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Post {
    /// Generates the full filename for the post, including its extension.
    ///
    /// The base name is determined by `name_type` (either ID or MD5),
    /// and the file extension is appended.
    #[inline]
    pub fn file_name(&self, name_type: NameType) -> String {
        let name = match name_type {
            NameType::ID => self.id.to_string(),
            NameType::MD5 => self.md5.to_string(),
        };

        format!("{}.{}", name, self.extension)
    }

    /// Generates the base name for the post, without its extension.
    ///
    /// The name is determined by `name_type` (either ID or MD5).
    #[inline]
    pub fn name(&self, name_type: NameType) -> String {
        match name_type {
            NameType::ID => self.id.to_string(),
            NameType::MD5 => self.md5.to_string(),
        }
    }

    /// Generates a sequential filename for the post using its ID, padded with leading zeros.
    ///
    /// # Arguments
    ///
    /// * `num_digits`: The total number of digits the ID part of the filename should have.
    #[inline]
    pub fn seq_file_name(&self, num_digits: usize) -> String {
        format!("{:0num_digits$}.{}", self.id, self.extension)
    }
}
