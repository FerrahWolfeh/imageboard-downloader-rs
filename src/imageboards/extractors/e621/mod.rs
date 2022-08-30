//! Post extractor for `https://e621.net`
//!
//! The e621 extractor has the following features:
//! - Authentication
//! - Tag blacklist (defined in user profile page)
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
//!     let mut ext = E621Extractor::new(&tags, safe_mode); // Initialize the extractor
//!
//!     ext.auth(true);
//!
//!     // Will iterate through all pages until it finds no more posts, then returns the list.
//!     let posts = ext.full_search().await.unwrap();
//!
//!     // Print all information collected
//!     println!("{:?}", posts);
//! }
//! ```
use crate::imageboards::auth::{auth_prompt, ImageboardConfig};
use crate::imageboards::extractors::e621::models::E621TopLevel;
use crate::imageboards::post::{rating::Rating, Post, PostQueue};
use crate::imageboards::ImageBoards;
use crate::{client, join_tags};
use ahash::AHashSet;
use async_trait::async_trait;
use cfg_if::cfg_if;
use log::debug;
use reqwest::Client;
use std::fmt::Display;
use std::time::Duration;
use tokio::time::{sleep, Instant};

use super::error::ExtractorError;
use super::{Auth, Extractor};

#[cfg(feature = "global_blacklist")]
use super::blacklist::GlobalBlacklist;

pub mod models;

//const _E621_FAVORITES: &str = "https://e621.net/favorites.json";

/// Main object to download posts
pub struct E621Extractor {
    client: Client,
    tags: Vec<String>,
    tag_string: String,
    pub auth_state: bool,
    pub auth: ImageboardConfig,
    safe_mode: bool,
    disable_blacklist: bool,
    total_removed: u64,
}

#[async_trait]
impl Extractor for E621Extractor {
    fn new<S>(tags: &[S], safe_mode: bool, disable_blacklist: bool) -> Self
    where
        S: ToString + Display,
    {
        // Use common client for all connections with a set User-Agent
        let client = client!(ImageBoards::E621.user_agent());

        // Set Safe mode status
        let safe_mode = safe_mode;

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
            client,
            tags: strvec,
            tag_string,
            auth_state: false,
            auth: Default::default(),
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

            if size < 320 || page == 100 {
                break;
            }

            page += 1;

            // debounce
            debug!("Debouncing API calls by 500 ms");
            sleep(Duration::from_millis(500)).await;
        }

        let fin = PostQueue {
            posts: fvec,
            tags: self.tags.to_vec(),
        };

        Ok(fin)
    }

    fn client(self) -> Client {
        self.client
    }

    fn total_removed(&self) -> u64 {
        self.total_removed
    }
}

#[async_trait]
impl Auth for E621Extractor {
    async fn auth(&mut self, prompt: bool) -> Result<(), ExtractorError> {
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
}

impl E621Extractor {
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

    #[inline]
    async fn blacklist_filter(&mut self, list: &mut Vec<Post>) -> Result<(), ExtractorError> {
        let original_size = list.len();
        let blacklist = &self.auth.user_data.blacklisted_tags;
        let mut removed = 0;

        let start = Instant::now();
        if !blacklist.is_empty() {
            list.retain(|c| !c.tags.iter().any(|s| blacklist.contains(s)));

            let bp = original_size - list.len();
            debug!("User blacklist removed {} posts", bp);
            removed += bp as u64;
        }

        cfg_if! {
            if #[cfg(feature = "global_blacklist")] {
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

                    if !tags.danbooru.is_empty() {
                        let fsize = list.len();
                        debug!("Removing posts with tags [{:?}]", tags.e621);
                        list.retain(|c| !c.tags.iter().any(|s| tags.e621.contains(s)));

                        let bp = fsize - list.len();
                        debug!("E621 blacklist removed {} posts", bp);
                        removed += bp as u64;
                    }
                }
            }
        }
        let end = Instant::now();
        debug!("Blacklist filtering took {:?}", end - start);
        debug!("Removed {} blacklisted posts", removed);
        self.total_removed += removed;

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
