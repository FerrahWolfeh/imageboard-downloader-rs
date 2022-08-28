//! Auth and download logic for `https://danbooru.donmai.us`
//&!
//! The danbooru downloader has the following features:
//! * Multiple simultaneous downloads.
//! * Authentication
//! * Tag blacklist (defined in user profile page)
//! * Safe mode (don't download NSFW posts)
//!
//! # Example usage
//!
//! ```rust
//! use std::path::PathBuf;
//! use imageboard_downloader::DanbooruDownloader;
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
//1 // Limit number of downloaded files
//! let limit = Some(100);
//!
//! // Initialize the downloader
//! let mut dl = DanbooruDownloader::new(&tags, output, sd, limit, auth, safe_mode, save_as_id).await?;
//!
//! // Download
//! dl.download().await?;
//! ```
use crate::imageboards::auth::ImageboardConfig;
use crate::imageboards::common::auth_prompt;
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
use serde_json::Value;
use std::io::{self, Write};
use tokio::time::Instant;

use super::error::ExtractorError;
use super::ImageBoardExtractor;

/// Main object to download posts
#[derive(Debug)]
pub struct DanbooruDownloader {
    client: Client,
    tags: Vec<String>,
    tag_string: String,
    pub auth_state: bool,
    pub auth: ImageboardConfig,
    safe_mode: bool,
}

#[async_trait]
impl ImageBoardExtractor for DanbooruDownloader {
    fn new(tags: &[String], safe_mode: bool) -> Self {
        // Use common client for all connections with a set User-Agent
        let client = client!(ImageBoards::Danbooru.user_agent());

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

    async fn search(&mut self, page: usize) -> Result<PostQueue, ExtractorError> {
        Self::validate_tags(self).await?;

        let posts = Self::get_post_list(self, page).await?;

        let qw = PostQueue {
            posts,
            tags: self.tags.to_vec(),
            user_blacklist: self.auth.user_data.blacklisted_tags.clone(),
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

            page += 1;

            print_found!(fvec);
        }
        println!();

        let fin = PostQueue {
            posts: fvec,
            tags: self.tags.to_vec(),
            user_blacklist: self.auth.user_data.blacklisted_tags.clone(),
        };
        Ok(fin)
    }
}

impl DanbooruDownloader {
    pub async fn auth(&mut self, prompt: bool) -> Result<(), Error> {
        // Try to authenticate, does nothing if auth flag is not set
        auth_prompt(prompt, ImageBoards::Danbooru, &self.client).await?;

        if let Some(creds) = ImageBoards::Danbooru.read_config_from_fs().await? {
            self.auth = creds;
            self.auth_state = true;
            return Ok(());
        }

        self.auth_state = false;
        Ok(())
    }

    async fn validate_tags(&self) -> Result<(), ExtractorError> {
        if self.tags.len() > 2 {
            return Err(ExtractorError::TooManyTags {
                current: self.tags.len(),
                max: 2,
            });
        };

        let count_endpoint = format!(
            "{}?tags={}",
            ImageBoards::Danbooru
                .post_count_url(self.safe_mode)
                .unwrap(),
            &self.tag_string
        );

        // Get an estimate of total posts and pages to search
        let request = if self.auth_state {
            debug!("[AUTH] Validating tags");
            self.client
                .get(count_endpoint)
                .basic_auth(&self.auth.username, Some(&self.auth.api_key))
        } else {
            debug!("Validating tags");
            self.client.get(count_endpoint)
        };

        let count = request.send().await?.json::<Value>().await?;

        if let Some(count) = count["counts"]["posts"].as_u64() {
            // Bail out if no posts are found
            if count == 0 {
                return Err(ExtractorError::ZeroPosts);
            }

            debug!("Found {} posts", count);
            Ok(())
        } else {
            Err(ExtractorError::InvalidServerResponse)
        }
    }

    async fn get_post_list(&self, page: usize) -> Result<Vec<Post>, ExtractorError> {
        // Check safe mode
        let url_mode = format!(
            "{}?tags={}",
            ImageBoards::Danbooru.post_url(self.safe_mode).unwrap(),
            &self.tag_string
        );

        // Fetch item list from page
        let req = if self.auth_state {
            debug!("[AUTH] Fetching posts from page {}", page);
            self.client
                .get(url_mode)
                .query(&[("page", page), ("limit", 200)])
                .basic_auth(&self.auth.username, Some(&self.auth.api_key))
        } else {
            debug!("Fetching posts from page {}", page);
            self.client
                .get(url_mode)
                .query(&[("page", page), ("limit", 200)])
        };

        let post_array = req.send().await?.json::<Value>().await?;

        let start_point = Instant::now();
        let posts: Vec<Post> = post_array
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["file_url"].as_str().is_some())
            .map(|c| {
                let mut tag_list = AHashSet::new();

                for i in c["tag_string"].as_str().unwrap().split(' ') {
                    tag_list.insert(i.to_string());
                }

                let rt = c["rating"].as_str().unwrap();
                let rating = if rt == "s" {
                    Rating::Questionable
                } else {
                    Rating::from_str(rt)
                };

                Post {
                    id: c["id"].as_u64().unwrap(),
                    md5: c["md5"].as_str().unwrap().to_string(),
                    url: c["file_url"].as_str().unwrap().to_string(),
                    extension: c["file_ext"].as_str().unwrap().to_string(),
                    tags: tag_list,
                    rating,
                }
            })
            .collect();
        let end_iter = Instant::now();

        debug!("List size: {}", posts.len());
        debug!("Post mapping took {:?}", end_iter - start_point);
        Ok(posts)
    }
}
