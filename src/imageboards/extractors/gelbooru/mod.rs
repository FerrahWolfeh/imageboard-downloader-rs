//! Download logic for Gelbooru-based imageboards
//!
//! The gelbooru downloader has the following features:
//! * Multiple simultaneous downloads.
//!
//! ***
//!
//! This downloader is compatible with these imageboards:
//! * `Imageboards::Rule34`
//! * `Imageboards::Realbooru`
//! * `Imageboards::Gelbooru`
//!
//! # Example usage
//!
//! ```rust
//! use std::path::PathBuf;
//! use imageboard_downloader::{ImageBoards, GelbooruDownloader};
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
//! // In this case, download from Rule34
//! let mut dl = GelbooruDownloader::new(ImageBoards::Rule34, &tags, output, sd, limit, save_as_id)?;
//!
//! // Download
//! dl.download().await?;
//! ```
use crate::imageboards::post::Post;
use crate::imageboards::queue::PostQueue;
use crate::imageboards::rating::Rating;
use crate::imageboards::ImageBoards;
use crate::{client, join_tags};
use crate::{extract_ext_from_url, print_found};
use ahash::AHashSet;
use async_trait::async_trait;
use colored::Colorize;
use log::debug;
use reqwest::Client;
use serde_json::Value;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;
use tokio::time::Instant;

use super::error::ExtractorError;
use super::ImageBoardExtractor;

pub struct GelbooruDownloader {
    active_imageboard: ImageBoards,
    client: Client,
    tags: Vec<String>,
    tag_string: String,
}

#[async_trait]
impl ImageBoardExtractor for GelbooruDownloader {
    async fn search(&mut self, page: usize) -> Result<PostQueue, ExtractorError> {
        Self::validate_tags(self).await?;

        let posts = Self::get_post_list(self, page).await?;

        let qw = PostQueue {
            posts,
            tags: self.tags.to_vec(),
            user_blacklist: Default::default(),
        };

        Ok(qw)
    }

    async fn full_search(&mut self) -> Result<PostQueue, ExtractorError> {
        Self::validate_tags(self).await?;

        let mut fvec = Vec::new();
        let mut page = 0;

        loop {
            let posts = Self::get_post_list(self, page).await?;
            let size = posts.len();

            if size == 0 {
                break;
            }

            fvec.extend(posts);

            if size < self.active_imageboard.max_post_limit() {
                break;
            }

            page += 1;

            print_found!(fvec);
            // debounce
            debug!("Debouncing API calls by 500 ms");
            thread::sleep(Duration::from_millis(500));
        }
        println!();

        let fin = PostQueue {
            posts: fvec,
            tags: self.tags.to_vec(),
            user_blacklist: Default::default(),
        };

        Ok(fin)
    }
}

impl GelbooruDownloader {
    pub fn new(tags: &[String], active_imageboard: ImageBoards) -> Self {
        // Use common client for all connections with a set User-Agent
        let client = client!(active_imageboard.user_agent());

        // Merge all tags in the URL format
        let tag_string = join_tags!(tags);

        Self {
            active_imageboard,
            client,
            tags: tags.to_vec(),
            tag_string,
        }
    }

    async fn validate_tags(&mut self) -> Result<(), ExtractorError> {
        let count_endpoint = format!(
            "{}&tags={}",
            self.active_imageboard.post_url(false).unwrap(),
            &self.tag_string
        );

        // Get an estimate of total posts and pages to search
        let request = self.client.get(&count_endpoint);

        debug!("Checking tags");

        let count = request.send().await?.json::<Value>().await?;

        // Bail out if no posts are found
        if let Some(res) = count.as_array() {
            if res.is_empty() {
                return Err(ExtractorError::ZeroPosts);
            }

            debug!("Tag list is valid");
            return Ok(());
        }

        if let Some(res) = count["post"].as_array() {
            if res.is_empty() {
                return Err(ExtractorError::ZeroPosts);
            }

            debug!("Tag list is valid");
            return Ok(());
        }

        Err(ExtractorError::InvalidServerResponse)
    }

    // This is mostly for sites running gelbooru 0.2, their xml API is way better than the JSON one
    async fn get_post_list(&self, page: usize) -> Result<Vec<Post>, ExtractorError> {
        let url_mode = format!(
            "{}&tags={}",
            self.active_imageboard.post_url(false).unwrap(),
            &self.tag_string
        );

        let items = &self
            .client
            .get(&url_mode)
            .query(&[("pid", page), ("limit", 1000)])
            .send()
            .await?
            .json::<Value>()
            .await?;

        if let Some(arr) = items.as_array() {
            let start = Instant::now();
            let posts: Vec<Post> = arr
                .iter()
                .filter(|f| f["hash"].as_str().is_some())
                .map(|f| {
                    let mut tags = AHashSet::new();

                    for i in f["tags"].as_str().unwrap().split(' ') {
                        tags.insert(i.to_string());
                    }

                    let rating = Rating::from_str(f["rating"].as_str().unwrap());

                    let file = f["image"].as_str().unwrap();

                    let md5 = f["hash"].as_str().unwrap().to_string();

                    let ext = extract_ext_from_url!(file);

                    let drop_url = if self.active_imageboard == ImageBoards::Rule34 {
                        f["file_url"].as_str().unwrap().to_string()
                    } else {
                        format!(
                            "https://realbooru.com/images/{}/{}.{}",
                            f["directory"].as_str().unwrap(),
                            &md5,
                            &ext
                        )
                    };

                    Post {
                        id: f["id"].as_u64().unwrap(),
                        url: drop_url,
                        md5,
                        extension: extract_ext_from_url!(file),
                        rating,
                        tags,
                    }
                })
                .collect();
            let end = Instant::now();

            debug!("List size: {}", posts.len());
            debug!("Post mapping took {:?}", end - start);

            return Ok(posts);
        }

        if let Some(it) = items["post"].as_array() {
            let start = Instant::now();
            let posts: Vec<Post> = it
                .iter()
                .filter(|i| i["file_url"].as_str().is_some())
                .map(|post| {
                    let url = post["file_url"].as_str().unwrap().to_string();
                    let mut tags = AHashSet::new();

                    for i in post["tags"].as_str().unwrap().split(' ') {
                        tags.insert(i.to_string());
                    }

                    Post {
                        id: post["id"].as_u64().unwrap(),
                        md5: post["md5"].as_str().unwrap().to_string(),
                        url: url.clone(),
                        extension: extract_ext_from_url!(url),
                        tags,
                        rating: Rating::from_str(post["rating"].as_str().unwrap()),
                    }
                })
                .collect();
            let end = Instant::now();

            debug!("List size: {}", posts.len());
            debug!("Post mapping took {:?}", end - start);

            return Ok(posts);
        }

        Err(ExtractorError::InvalidServerResponse)
    }
}
