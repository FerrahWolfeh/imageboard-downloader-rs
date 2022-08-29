//! Post extractor for `https://konachan.com` and other Moebooru imageboards
//!
//! The moebooru extractor has the following features:
//! - Native safe mode (don't download NSFW posts)
//!
//! # Example basic usage
//!
//! ```rust
//! use imageboard_downloader::*;
//!
//! async fn fetch_posts() {
//!     let tags = ["umbreon".to_string(), "espeon".to_string()];
//!     
//!     let safe_mode = true; // Set to true to download posts from safebooru
//!
//!     let mut ext = MoebooruExtractor::new(&tags, safe_mode); // Initialize the extractor
//!
//!     // Will iterate through all pages until it finds no more posts, then returns the list.
//!     let posts = ext.full_search().await.unwrap();
//!
//!     // Print all information collected
//!     println!("{:?}", posts);
//! }
//! ```
use crate::imageboards::extractors::error::ExtractorError;
use crate::imageboards::extractors::moebooru::models::KonachanPost;
use crate::imageboards::post::{rating::Rating, Post, PostQueue};
use crate::imageboards::ImageBoards;
use crate::{client, extract_ext_from_url, join_tags, print_found};
use ahash::AHashSet;
use async_trait::async_trait;
use cfg_if::cfg_if;
use colored::Colorize;
use log::debug;
use reqwest::Client;
use std::io::{self, Write};
use tokio::time::Instant;

#[cfg(feature = "global_blacklist")]
use crate::imageboards::blacklist::GlobalBlacklist;

use super::Extractor;

mod models;

pub struct MoebooruExtractor {
    client: Client,
    tags: Vec<String>,
    tag_string: String,
    safe_mode: bool,
    disable_blacklist: bool,
    total_removed: u64,
}

#[async_trait]
impl Extractor for MoebooruExtractor {
    fn new(tags: &[String], safe_mode: bool, disable_blacklist: bool) -> Self {
        // Use common client for all connections with a set User-Agent
        let client = client!(ImageBoards::Konachan.user_agent());

        // Merge all tags in the URL format
        let tag_string = join_tags!(tags);

        // Set Safe mode status
        let safe_mode = safe_mode;

        Self {
            client,
            tags: tags.to_vec(),
            tag_string,
            safe_mode,
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
            user_blacklist: Default::default(),
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
                page + n
            } else {
                page
            };

            let mut posts = Self::get_post_list(self, position).await?;
            let size = posts.len();

            if size == 0 {
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

            if size < 320 || page == 100 {
                break;
            }

            page += 1;

            print_found!(fvec);
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

impl MoebooruExtractor {
    async fn validate_tags(&self) -> Result<(), ExtractorError> {
        let count_endpoint = format!(
            "{}?tags={}",
            ImageBoards::Konachan.post_url(self.safe_mode).unwrap(),
            &self.tag_string
        );

        // Get an estimate of total posts and pages to search
        let count = &self
            .client
            .get(&count_endpoint)
            .send()
            .await?
            .json::<Vec<KonachanPost>>()
            .await?;

        // Bail out if no posts are found
        if count.is_empty() {
            return Err(ExtractorError::ZeroPosts);
        }
        debug!("Tag list is valid");

        Ok(())
    }

    #[inline]
    async fn blacklist_filter(&mut self, list: &mut Vec<Post>) -> Result<(), ExtractorError> {
        cfg_if! {
            if #[cfg(feature = "global_blacklist")] {
                let mut removed = 0;
                let start = Instant::now();
                let gbl = GlobalBlacklist::get().await.unwrap();

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

                    if !tags.danbooru.is_empty() {
                        let fsize = list.len();
                        debug!("Removing posts with tags [{:?}]", tags.konachan);
                        list.retain(|c| !c.tags.iter().any(|s| tags.konachan.contains(s)));

                        let bp = fsize - list.len();
                        debug!("Konachan blacklist removed {} posts", bp);
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

    async fn get_post_list(&self, page: usize) -> Result<Vec<Post>, ExtractorError> {
        // Get URL
        let count_endpoint = format!(
            "{}?tags={}",
            ImageBoards::Konachan.post_url(self.safe_mode).unwrap(),
            &self.tag_string
        );

        let items = &self
            .client
            .get(&count_endpoint)
            .query(&[("page", page), ("limit", 100)])
            .send()
            .await?
            .json::<Vec<KonachanPost>>()
            .await?;

        let post_list: Vec<Post> = items
            .iter()
            .filter(|c| c.file_url.is_some())
            .map(|c| {
                let url = c.file_url.clone().unwrap();

                let mut tags = AHashSet::new();

                for i in c.tags.split(' ') {
                    tags.insert(i.to_string());
                }

                Post {
                    id: c.id.unwrap(),
                    url: url.clone(),
                    md5: c.md5.clone().unwrap(),
                    extension: extract_ext_from_url!(url),
                    tags,
                    rating: Rating::from_str(&c.rating),
                }
            })
            .collect();

        Ok(post_list)
    }
}
