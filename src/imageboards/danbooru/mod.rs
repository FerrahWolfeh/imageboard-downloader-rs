//! Auth and download logic for `https://danbooru.donmai.us`
//!
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
use crate::imageboards::common::{generate_out_dir, try_auth, Counters};
use crate::imageboards::post::Post;
use crate::imageboards::queue::DownloadQueue;
use crate::imageboards::ImageBoards;
use crate::progress_bars::ProgressArcs;
use crate::{client, finish_and_print_results, join_tags};
use ahash::AHashSet;
use anyhow::{bail, Error};
use colored::Colorize;
use log::debug;
use reqwest::Client;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::fs::create_dir_all;
use tokio::time::Instant;

/// Main object to download posts
pub struct DanbooruDownloader {
    item_count: u64,
    page_count: f32,
    client: Client,
    concurrent_downloads: usize,
    download_limit: Option<usize>,
    tag_list: Vec<String>,
    tag_string: String,
    out_dir: PathBuf,
    safe_mode: bool,
    save_as_id: bool,
    counters: Counters,
    blacklisted_posts: u64,
}

impl DanbooruDownloader {
    pub async fn new(
        tags: &[String],
        out_dir: Option<PathBuf>,
        concurrent_downs: usize,
        item_limit: Option<usize>,
        auth_state: bool,
        safe_mode: bool,
        save_as_id: bool,
    ) -> Result<Self, Error> {
        if tags.len() > 2 {
            bail!("Danbooru downloader currently doesn't support more than 2 tags")
        };
        // Use common client for all connections with a set User-Agent
        let client = client!(ImageBoards::Danbooru.user_agent());

        // Join tags to a url format in case there's more than one
        let tag_string = join_tags!(tags);

        // Place downloaded items in current dir or in /tmp
        let out = generate_out_dir(out_dir, &tags, ImageBoards::Danbooru)?;

        // Try to authenticate, does nothing if auth flag is not set
        try_auth(auth_state, ImageBoards::Danbooru, &client).await?;

        Ok(Self {
            item_count: 0,
            page_count: 0.0,
            concurrent_downloads: concurrent_downs,
            download_limit: item_limit,
            tag_list: Vec::from(tags),
            tag_string,
            client,
            out_dir: out,
            safe_mode,
            save_as_id,
            blacklisted_posts: 0,
            counters: Counters {
                total_mtx: Arc::new(Mutex::new(0)),
                downloaded_mtx: Arc::new(Mutex::new(0)),
            },
        })
    }

    async fn get_post_count(&mut self, auth_creds: &Option<ImageboardConfig>) -> Result<(), Error> {
        let count_endpoint = format!(
            "{}?tags={}",
            ImageBoards::Danbooru
                .post_count_url(self.safe_mode)
                .unwrap(),
            &self.tag_string
        );

        // Get an estimate of total posts and pages to search
        let count = if let Some(data) = auth_creds {
            debug!("[AUTH] Fetching post count");
            self.client
                .get(count_endpoint)
                .basic_auth(&data.username, Some(&data.api_key))
                .send()
                .await?
                .json::<Value>()
                .await?
        } else {
            debug!("Fetching post count");
            self.client
                .get(count_endpoint)
                .send()
                .await?
                .json::<Value>()
                .await?
        };

        if let Some(count) = count["counts"]["posts"].as_u64() {
            // Bail out if no posts are found
            if count == 0 {
                bail!("No posts found for tag selection!")
            }

            if let Some(num) = self.download_limit {
                self.item_count = num as u64;
                self.page_count = (num as f32 / 200.0).ceil();
            } else {
                self.item_count = count;
                self.page_count = (self.item_count as f32 / 200.0).ceil();
            }

            debug!(
                "{} Posts for tag list '{:?}'",
                &self.item_count, &self.tag_list
            );
        } else {
            bail!("Danbooru returned a malformed JSON response while fetching post count.")
        }
        Ok(())
    }

    pub async fn download(&mut self) -> Result<(), Error> {
        // Get auth data
        let auth_res = ImageBoards::Danbooru.read_config_from_fs().await?;

        // Generate post count data
        Self::get_post_count(self, &auth_res).await?;

        // Create output dir
        create_dir_all(&self.out_dir).await?;

        // Setup global progress bar
        let bars = ProgressArcs::initialize(self.item_count, ImageBoards::Danbooru);

        // Begin downloading all posts per page
        for i in 1..=self.page_count as u64 {
            bars.main.set_message(format!("Page {i}"));

            // Check safe mode
            let url_mode = format!(
                "{}?tags={}",
                ImageBoards::Danbooru.post_url(self.safe_mode).unwrap(),
                &self.tag_string
            );

            // Fetch item list from page
            let jj = if let Some(data) = &auth_res {
                debug!("[AUTH] Fetching posts from page {}", i);
                self.client
                    .get(url_mode)
                    .query(&[("page", &i.to_string()), ("limit", &200.to_string())])
                    .basic_auth(&data.username, Some(&data.api_key))
                    .send()
                    .await?
                    .json::<Value>()
                    .await?
            } else {
                debug!("Fetching posts from page {}", i);
                self.client
                    .get(url_mode)
                    .query(&[("page", &i.to_string()), ("limit", &200.to_string())])
                    .send()
                    .await?
                    .json::<Value>()
                    .await?
            };

            let start_point = Instant::now();
            let mut posts: Vec<Post> = jj
                .as_array()
                .unwrap()
                .iter()
                .filter(|c| c["file_url"].as_str().is_some())
                .map(|c| {
                    let mut tag_list = AHashSet::new();

                    for i in c["tag_string"].as_str().unwrap().split(' ') {
                        if !i.contains("//") {
                            tag_list.insert(i.to_string());
                        }
                    }

                    Post {
                        id: c["id"].as_u64().unwrap(),
                        md5: c["md5"].as_str().unwrap().to_string(),
                        url: c["file_url"].as_str().unwrap().to_string(),
                        extension: c["file_ext"].as_str().unwrap().to_string(),
                        tags: tag_list,
                    }
                })
                .collect();
            debug!("List size: {}", posts.len());

            if let Some(auth) = &auth_res {
                let original_count = posts.len();
                posts.retain(|c| {
                    c.tags
                        .iter()
                        .any(|s| !auth.user_data.blacklisted_tags.contains(s))
                });

                let bp = original_count - posts.len();

                debug!("Removed {} blacklisted posts", bp);
                self.blacklisted_posts += bp as u64;
            }
            let end_iter = Instant::now();
            debug!("Post mapping took {:?}", end_iter - start_point);

            // Download everything got in the above function
            let queue = DownloadQueue::new(
                posts,
                self.concurrent_downloads,
                self.download_limit,
                self.counters.clone(),
            );

            queue
                .download_all(
                    &self.client,
                    &self.out_dir,
                    bars.clone(),
                    ImageBoards::Danbooru,
                    self.save_as_id,
                )
                .await?;
        }

        finish_and_print_results!(bars, self, auth_res);
        Ok(())
    }
}
