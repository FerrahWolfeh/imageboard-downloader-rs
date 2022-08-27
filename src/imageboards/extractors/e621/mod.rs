//! Auth and download logic for `https://e621.net`
//!
//! The e621 downloader has the following features:
//! * Multiple simultaneous downloads.
//! * Authentication
//! * Tag blacklist (defined in user profile page)
//! * Safe mode (don't download NSFW posts)
//!
//! # Example usage
//!
//! ```rust
//! use std::path::PathBuf;
//! use imageboard_downloader::E621Downloader;
//!
//! // Input tags
//! let tags = vec!["umbreon".to_string(), "espeon".to_string()];
//!
//! // Dir where all will be saved
//! let output = Some(PathBuf::from("./"));
//!
//! // Number of simultaneous downloads
//! let sd = 3;
//!
//! // Disable download of NSFW posts
//! let safe_mode = true;
//!
//! // Login to the imageboard (only needs to be true once)
//! let auth = true;
//!
//! // Save files with as <post_id>.png rather than <image_md5>.png
//! let save_as_id = false;
//!
//! // Limit number of downloaded files
//! let limit = Some(100);
//!
//! // Initialize the downloader
//! let mut dl = E621Downloader::new(&tags, output, sd, limit, auth, safe_mode, save_as_id).await?;
//!
//! // Download
//! dl.download().await?;
//! ```
use crate::imageboards::auth::ImageboardConfig;
use crate::imageboards::common::auth_prompt;
use crate::imageboards::extractors::e621::models::E621TopLevel;
use crate::imageboards::post::Post;
use crate::imageboards::queue::PostQueue;
use crate::imageboards::rating::Rating;
use crate::imageboards::ImageBoards;
use crate::{client, join_tags, print_found};
use ahash::AHashSet;
use anyhow::Error;
use async_trait::async_trait;
use colored::Colorize;
use log::debug;
use reqwest::Client;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;
use tokio::time::Instant;

use super::error::ExtractorError;
use super::ImageBoardExtractor;

pub mod models;

//const _E621_FAVORITES: &str = "https://e621.net/favorites.json";

/// Main object to download posts
pub struct E621Downloader {
    client: Client,
    tags: Vec<String>,
    tag_string: String,
    pub auth_state: bool,
    pub auth: ImageboardConfig,
    safe_mode: bool,
}

#[async_trait]
impl ImageBoardExtractor for E621Downloader {
    async fn search(&mut self, page: usize) -> Result<PostQueue, ExtractorError> {
        Self::validate_tags(self).await?;

        let posts = Self::get_post_list(self, page).await?;

        let qw = PostQueue {
            tags: self.tags.to_vec(),
            posts,
        };

        Ok(qw)
    }

    async fn full_search(&mut self) -> Result<PostQueue, ExtractorError> {
        Self::validate_tags(self).await?;

        let mut fvec = Vec::new();
        let mut page = 1;

        loop {
            let posts = Self::get_post_list(self, page).await?;
            let size = posts.len();

            if size == 0 {
                break;
            }

            fvec.extend(posts);

            if size < 320 {
                break;
            }

            page += 1;

            print_found!(fvec);
        }
        println!();

        let fin = PostQueue {
            tags: self.tags.to_vec(),
            posts: fvec,
        };

        Ok(fin)
    }
}

impl E621Downloader {
    pub fn new(tags: &[String], safe_mode: bool) -> Self {
        // Use common client for all connections with a set User-Agent
        let client = client!(ImageBoards::E621.user_agent());

        // Merge all tags in the URL format
        let tag_string = join_tags!(tags);

        // Set Safe mode status
        let safe_mode = safe_mode;

        Self {
            client,
            tags: tags.to_vec(),
            tag_string,
            auth_state: false,
            auth: Default::default(),
            safe_mode,
        }
    }

    pub async fn auth(&mut self, prompt: bool) -> Result<(), Error> {
        // Try to authenticate, does nothing if auth flag is not set
        auth_prompt(prompt, ImageBoards::E621, &self.client).await?;

        if let Some(creds) = ImageBoards::E621.read_config_from_fs().await? {
            self.auth = creds;
            self.auth_state = true;
            return Ok(());
        }

        self.auth_state = false;
        Ok(())
    }

    async fn validate_tags(&self) -> Result<(), ExtractorError> {
        let count_endpoint = format!(
            "{}?tags={}",
            ImageBoards::E621.post_url(self.safe_mode).unwrap(),
            &self.tag_string
        );

        // Get an estimate of total posts and pages to search
        let request = if self.auth_state {
            debug!("[AUTH] Checking tags");
            self.client
                .get(&count_endpoint)
                .basic_auth(&self.auth.username, Some(&self.auth.api_key))
        } else {
            debug!("Checking tags");
            self.client.get(&count_endpoint)
        };

        let count = request.send().await?.json::<E621TopLevel>().await?;

        // Bail out if no posts are found
        if count.posts.is_empty() {
            return Err(ExtractorError::ZeroPosts);
        }
        debug!("Tag list is valid");

        Ok(())
    }

    pub async fn get_post_list(&self, page: usize) -> Result<Vec<Post>, ExtractorError> {
        // Check safe mode
        let url_mode = format!(
            "{}?tags={}",
            ImageBoards::E621.post_url(self.safe_mode).unwrap(),
            &self.tag_string
        );

        let req = if self.auth_state {
            debug!("[AUTH] Fetching posts from page {}", page);
            self.client
                .get(&url_mode)
                .query(&[("page", page), ("limit", 320)])
                .basic_auth(&self.auth.username, Some(&self.auth.api_key))
        } else {
            debug!("Fetching posts from page {}", page);
            self.client
                .get(&url_mode)
                .query(&[("page", page), ("limit", 320)])
        };

        let items = req.send().await?.json::<E621TopLevel>().await?;

        let start_point = Instant::now();
        let post_list: Vec<Post> = items
            .posts
            .iter()
            .filter(|c| c.file.url.is_some())
            .map(|c| {
                let mut tag_list = AHashSet::new();
                tag_list.extend(c.tags.character.iter().cloned());
                tag_list.extend(c.tags.artist.iter().cloned());
                tag_list.extend(c.tags.general.iter().cloned());
                tag_list.extend(c.tags.invalid.iter().cloned());
                tag_list.extend(c.tags.copyright.iter().cloned());
                tag_list.extend(c.tags.lore.iter().cloned());
                tag_list.extend(c.tags.meta.iter().cloned());
                tag_list.extend(c.tags.species.iter().cloned());

                Post {
                    id: c.id.unwrap(),
                    url: c.file.url.clone().unwrap(),
                    md5: c.file.md5.clone().unwrap(),
                    extension: c.file.ext.clone().unwrap(),
                    tags: tag_list,
                    rating: Rating::from_str(&c.rating),
                }
            })
            .collect();
        let end_point = Instant::now();

        debug!("List size: {}", post_list.len());
        debug!("Post mapping took {:?}", end_point - start_point);
        Ok(post_list)
    }
}
