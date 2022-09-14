//! Main representation of a imageboard post
//!
//! # Post
//! A [`Post` struct](Post) is a generic representation of an imageboard post.
//!
//! Most imageboard APIs have a common set of info from the files we want to download.
use ahash::AHashSet;
use serde::{Deserialize, Serialize};

use std::{cmp::Ordering, ops::Not};

use crate::ImageBoards;

use self::rating::Rating;

pub mod error;
pub mod rating;

#[derive(Debug, Clone, Copy)]
pub enum NameType {
    ID,
    MD5,
}

impl Not for NameType {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            NameType::ID => NameType::MD5,
            NameType::MD5 => NameType::ID,
        }
    }
}

impl PartialEq for NameType {
    fn eq(&self, other: &Self) -> bool {
        core::mem::discriminant(self) == core::mem::discriminant(other)
    }
}

/// Queue that combines all posts collected, with which tags and with a user-defined blacklist in case an Extractor implements [Auth](crate::imageboards::extractors::Auth).
#[derive(Debug)]
pub struct PostQueue {
    pub imageboard: ImageBoards,
    /// A list containing all `Post`s collected.
    pub posts: Vec<Post>,
    /// The tags used to search the collected posts.
    pub tags: Vec<String>,
}

/// Catchall model for the necessary parts of the imageboard post to properly identify, download and save it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    /// ID number of the post given by the imageboard
    pub id: u64,
    /// Direct URL of the original image file located inside the imageboard's server
    pub url: String,
    /// Instead of calculating the downloaded file's MD5 hash on the fly, it uses the one provided by the API.
    pub md5: String,
    /// The original file extension provided by the imageboard.
    ///
    /// ```https://konachan.com``` (Moebooru) and some other imageboards don't provide this field. So, additional work is required to get the file extension from the url
    pub extension: String,
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
    pub tags: AHashSet<String>,
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

impl Eq for Post {}

impl Post {
    #[inline]
    pub fn file_name(&self, name_type: NameType) -> String {
        let name = match name_type {
            NameType::ID => self.id.to_string(),
            NameType::MD5 => self.md5.to_string(),
        };

        format!("{}.{}", name, self.extension)
    }
}
