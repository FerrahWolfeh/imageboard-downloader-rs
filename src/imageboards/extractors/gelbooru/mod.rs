//! Post extractor for Gelbooru-based imageboards
//!
//! This extractor is compatible with these imageboards:
//! * `Imageboards::Rule34`
//! * `Imageboards::Realbooru`
//! * `Imageboards::Gelbooru`
//!
//! # Example usage
//!
//! ```rust
//! use imageboard_downloader::*;
//!
//! async fn fetch_posts() {
//!     let tags = ["umbreon".to_string(), "espeon".to_string()];
//!
//!     // Note: Safe mode with this extractor is ignored
//!     let mut ext = GelbooruExtractor::new(&tags, false) // Initialize the extractor
//!         .set_imageboard(ImageBoards::Rule34); // Set to extract from Rule34
//!
//!
//!     // Will iterate through all pages until it finds no more posts, then returns the list.
//!     let posts = ext.full_search().await.unwrap();
//!
//!     // Print all information collected
//!     println!("{:?}", posts);
//! }
//! ```
use crate::imageboards::post::{rating::Rating, Post, PostQueue};
use crate::imageboards::ImageBoards;
use crate::{client, join_tags};
use crate::{extract_ext_from_url, print_found};
use ahash::AHashSet;
use async_trait::async_trait;
use cfg_if::cfg_if;
use colored::Colorize;
use log::debug;
use reqwest::Client;
use serde_json::Value;
use std::fmt::Display;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;
use tokio::time::Instant;

#[cfg(feature = "global_blacklist")]
use super::blacklist::GlobalBlacklist;

use super::error::ExtractorError;
use super::Extractor;

pub struct GelbooruExtractor {
    active_imageboard: ImageBoards,
    client: Client,
    tags: Vec<String>,
    tag_string: String,
    disable_blacklist: bool,
    total_removed: u64,
}

#[async_trait]
impl Extractor for GelbooruExtractor {
    #[allow(unused_variables)]
    fn new<S>(tags: &[S], safe_mode: bool, disable_blacklist: bool) -> Self
    where
        S: ToString + Display,
    {
        // Use common client for all connections with a set User-Agent
        let client = Client::builder()
            .user_agent(ImageBoards::Rule34.user_agent())
            .build()
            .unwrap();

        let strvec: Vec<String> = tags
            .iter()
            .map(|t| {
                let st: String = t.to_string();
                st
            })
            .collect();

        // Merge all tags in the URL format
        let tag_string = join_tags!(strvec);

        Self {
            active_imageboard: ImageBoards::Rule34,
            client,
            tags: strvec,
            tag_string,
            disable_blacklist,
            total_removed: 0,
        }
    }

    async fn search(&mut self, page: usize) -> Result<PostQueue, ExtractorError> {
        Self::validate_tags(self).await?;

        let posts = Self::get_post_list(self, page).await?;

        let qw = PostQueue {
            posts,
            tags: self.tags.to_vec(),
        };

        Ok(qw)
    }

    async fn full_search(
        &mut self,
        start_page: Option<usize>,
        limit: Option<usize>,
    ) -> Result<PostQueue, ExtractorError> {
        Self::validate_tags(self).await?;

        let mut fvec = Vec::new();

        let mut page = 1;

        loop {
            let position = if let Some(n) = start_page {
                page + n - 1
            } else {
                page - 1
            };

            let mut posts = Self::get_post_list(self, position).await?;
            let size = posts.len();

            if size == 0 {
                println!();
                break;
            }

            if !self.disable_blacklist {
                self.blacklist_filter(&mut posts).await?;
            }

            fvec.extend(posts);

            if let Some(num) = limit {
                if fvec.len() >= num {
                    break;
                }
            }

            if size < self.active_imageboard.max_post_limit() || page == 100 {
                break;
            }

            page += 1;

            print_found!(fvec);
            //debounce
            debug!("Debouncing API calls by 500 ms");
            thread::sleep(Duration::from_millis(500));
        }
        println!();

        let fin = PostQueue {
            posts: fvec,
            tags: self.tags.to_vec(),
        };

        Ok(fin)
    }

    fn client(&mut self, client: Client) {
        self.client = client;
    }
}

impl GelbooruExtractor {
    /// Sets the imageboard to extract posts from
    ///
    /// If not set, defaults to `ImageBoards::Rule34`
    pub fn set_imageboard(self, imageboard: ImageBoards) -> Result<Self, ExtractorError> {
        let imageboard = match imageboard {
            ImageBoards::Gelbooru | ImageBoards::Realbooru | ImageBoards::Rule34 => imageboard,
            _ => {
                return Err(ExtractorError::InvalidImageboard {
                    imgboard: imageboard.to_string(),
                })
            }
        };
        let client = client!(imageboard.user_agent());

        Ok(Self {
            active_imageboard: imageboard,
            client,
            tags: self.tags,
            tag_string: self.tag_string,
            disable_blacklist: self.disable_blacklist,
            total_removed: self.total_removed,
        })
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

    #[inline]
    async fn blacklist_filter(&mut self, list: &mut Vec<Post>) -> Result<(), ExtractorError> {
        cfg_if! {
            if #[cfg(feature = "global_blacklist")] {
                let mut removed = 0;
                let start = Instant::now();
                let gbl = GlobalBlacklist::get().await?;

                if let Some(tags) = gbl.blacklist {
                    if !tags.global.is_empty() {
                        let fsize = list.len();
                        debug!("Removing posts with tags [{:?}]", tags);
                        list.retain(|c| !c.tags.iter().any(|s| tags.global.contains(s)));

                        let bp = fsize - list.len();
                        debug!("Global blacklist removed {} posts", bp);
                        removed += bp as u64;
                    } else {
                        debug!("Global blacklist is empty")
                    }

                    let special_tags = match self.active_imageboard {
                        ImageBoards::Rule34 => {
                            tags.rule34
                        }
                        ImageBoards::Gelbooru => {
                            tags.gelbooru
                        }
                        ImageBoards::Realbooru => {
                            tags.realbooru
                        }
                        _ => return Err(ExtractorError::ImpossibleBehavior)
                    };

                    if !special_tags.is_empty() {
                        let fsize = list.len();
                        debug!("Removing posts with tags [{:?}]", special_tags);
                        list.retain(|c| !c.tags.iter().any(|s| special_tags.contains(s)));

                        let bp = fsize - list.len();
                        debug!("Blacklist removed {} posts", bp);
                        removed += bp as u64;
                    }
                }

                let end = Instant::now();
                debug!("Blacklist filtering took {:?}", end - start);
                debug!("Removed {} blacklisted posts", removed);
                self.total_removed += removed;
            }
        }
        Ok(())
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
