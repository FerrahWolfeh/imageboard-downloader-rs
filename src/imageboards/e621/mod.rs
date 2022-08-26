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
use crate::imageboards::common::{generate_out_dir, try_auth, Counters};
use crate::imageboards::e621::models::E621TopLevel;
use crate::imageboards::post::Post;
use crate::imageboards::queue::DownloadQueue;
use crate::imageboards::rating::Rating;
use crate::imageboards::ImageBoards;
use crate::progress_bars::ProgressArcs;
use crate::{client, finish_and_print_results, join_tags};
use ahash::AHashSet;
use anyhow::{bail, Error};
use colored::Colorize;
use log::debug;
use reqwest::Client;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tokio::fs::create_dir_all;

pub mod models;

//const _E621_FAVORITES: &str = "https://e621.net/favorites.json";

/// Main object to download posts
pub struct E621Downloader {
    item_count: u64,
    client: Client,
    tag_string: String,
    //tag_list: Vec<String>,
    concurrent_downloads: usize,
    posts_endpoint: String,
    out_dir: PathBuf,
    safe_mode: bool,
    save_as_id: bool,
    download_limit: Option<usize>,
    counters: Counters,
    blacklisted_posts: usize,
}

impl E621Downloader {
    pub async fn new(
        tags: &[String],
        out_dir: Option<PathBuf>,
        concurrent_downs: usize,
        download_limit: Option<usize>,
        auth_state: bool,
        safe_mode: bool,
        save_as_id: bool,
    ) -> Result<Self, Error> {
        // Use common client for all connections with a set User-Agent (e621 requires this)
        let client = client!(ImageBoards::E621.user_agent());

        // Join tags to a url format in case there's more than one
        let tag_string = join_tags!(tags);

        // Place downloaded items in current dir or in /tmp
        let out = generate_out_dir(out_dir, &tags, ImageBoards::E621)?;

        // Try to authenticate, does nothing if auth flag is not set
        try_auth(auth_state, ImageBoards::E621, &client).await?;

        Ok(Self {
            item_count: 0,
            client,
            tag_string,
            //tag_list: Vec::from(tags),
            concurrent_downloads: concurrent_downs,
            posts_endpoint: "".to_string(),
            out_dir: out,
            safe_mode,
            save_as_id,
            download_limit,
            counters: Counters {
                total_mtx: Arc::new(Mutex::new(0)),
                downloaded_mtx: Arc::new(Mutex::new(0)),
            },
            blacklisted_posts: 0,
        })
    }

    async fn check_tag_list(&mut self, auth_creds: &Option<ImageboardConfig>) -> Result<(), Error> {
        let count_endpoint = format!(
            "{}?tags={}",
            ImageBoards::E621.post_url(self.safe_mode).unwrap(),
            &self.tag_string
        );

        // Get an estimate of total posts and pages to search
        let count = if let Some(data) = auth_creds {
            debug!("[AUTH] Checking tags");
            self.client
                .get(&count_endpoint)
                .basic_auth(&data.username, Some(&data.api_key))
                .send()
                .await?
                .json::<E621TopLevel>()
                .await?
        } else {
            debug!("Checking tags");
            self.client
                .get(&count_endpoint)
                .send()
                .await?
                .json::<E621TopLevel>()
                .await?
        };

        // Bail out if no posts are found
        if count.posts.is_empty() {
            bail!("No posts found for tag selection!")
        }
        debug!("Tag list is valid");

        // Fill memory with standard post count just to initialize the progress bar
        if let Some(num) = self.download_limit {
            self.item_count = num as u64;
        } else {
            self.item_count = count.posts.len() as u64;
        }

        self.posts_endpoint = count_endpoint;

        Ok(())
    }

    pub async fn download(&mut self) -> Result<(), Error> {
        // Get auth data
        let auth_res = ImageBoards::E621.read_config_from_fs().await?;

        // Generate post count data
        Self::check_tag_list(self, &auth_res).await?;

        // Create output dir
        create_dir_all(&self.out_dir).await?;

        // In case the limit is set, we can initialize the main progress bar with a known value
        let initial_len = if let Some(n) = self.download_limit {
            n as u64
        } else {
            0
        };

        // Setup global progress bar
        let bars = ProgressArcs::initialize(initial_len, ImageBoards::E621);

        // Keep track of pages already downloaded
        let mut page = 0;

        // Begin downloading all posts per page
        // Since e621 doesn't give a precise number of posts, we'll need to go through the pages until there's no more posts left to show
        loop {
            page += 1;

            bars.main.set_message(format!("Page {page}"));

            let items = if let Some(data) = &auth_res {
                debug!("[AUTH] Fetching posts from page {}", page);
                self.client
                    .get(&self.posts_endpoint)
                    .query(&[("page", page), ("limit", 320)])
                    .basic_auth(&data.username, Some(&data.api_key))
                    .send()
                    .await?
                    .json::<E621TopLevel>()
                    .await?
            } else {
                debug!("Fetching posts from page {}", page);
                self.client
                    .get(&self.posts_endpoint)
                    .query(&[("page", page), ("limit", 320)])
                    .send()
                    .await?
                    .json::<E621TopLevel>()
                    .await?
            };

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
                        rating: Rating::from_str(&c.rating).unwrap(),
                    }
                })
                .collect();

            debug!("List size: {}", post_list.len());

            let list_len = post_list.len() as u64;

            if let Some(dl) = self.download_limit {
                if list_len < dl as u64 {
                    self.item_count = post_list.len() as u64
                } else {
                    self.item_count = self.download_limit.unwrap() as u64
                }
            } else {
                self.item_count = list_len;

                bars.main
                    .inc_length(list_len - self.blacklisted_posts as u64);
            };

            if self.item_count != 0 {
                let mut queue = DownloadQueue::new(
                    post_list,
                    self.concurrent_downloads,
                    self.download_limit,
                    self.counters.clone(),
                );

                if let Some(auth) = &auth_res {
                    queue.blacklist_filter(&auth.user_data.blacklisted_tags);
                }

                queue
                    .download(
                        &self.client,
                        &self.out_dir,
                        bars.clone(),
                        ImageBoards::E621,
                        self.save_as_id,
                    )
                    .await?;
                self.blacklisted_posts += queue.blacklisted_ct();
            }

            if let Some(n) = self.download_limit {
                if n == *self.counters.total_mtx.lock().unwrap() {
                    break;
                }
            }

            if self.item_count < 320 {
                break;
            }
        }

        finish_and_print_results!(bars, self, auth_res);
        Ok(())
    }
}
