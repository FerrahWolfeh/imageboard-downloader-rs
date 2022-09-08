//! Post extractor for `https://e621.net`
//!
//! The e621 extractor has the following features:
//! - Authentication
//! - Native blacklist (defined in user profile page)
//!
use crate::imageboards::auth::{auth_prompt, ImageboardConfig};
use crate::imageboards::extractors::blacklist::blacklist_filter;
use crate::imageboards::extractors::e621::models::E621TopLevel;
use crate::imageboards::post::{rating::Rating, Post, PostQueue};
use crate::imageboards::ImageBoards;
use crate::{client, join_tags};
use ahash::AHashSet;
use async_trait::async_trait;
use log::debug;
use reqwest::Client;
use std::fmt::Display;
use std::time::Duration;
use tokio::time::{sleep, Instant};

use super::error::ExtractorError;
use super::{Auth, Extractor};

pub mod models;

//const _E621_FAVORITES: &str = "https://e621.net/favorites.json";

/// Main object to download posts
pub struct E621Extractor {
    client: Client,
    tags: Vec<String>,
    tag_string: String,
    auth_state: bool,
    auth: ImageboardConfig,
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
        let client = client!(ImageBoards::E621);

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
        debug!("Tag List: {}", tag_string);

        Self {
            client,
            tags: strvec,
            tag_string,
            auth_state: false,
            auth: ImageboardConfig::default(),
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
            tags: self.tags.clone(),
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
                self.total_removed += blacklist_filter(
                    ImageBoards::E621,
                    &mut posts,
                    &self.auth.user_data.blacklisted_tags,
                    self.safe_mode,
                )
                .await?;
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

        fvec.sort();
        fvec.reverse();

        let fin = PostQueue {
            posts: fvec,
            tags: self.tags.clone(),
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
        let count_endpoint = format!("{}?tags={}", ImageBoards::E621.post_url(), &self.tag_string);

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
        let url = format!("{}?tags={}", ImageBoards::E621.post_url(), &self.tag_string);

        let req = if self.auth_state {
            debug!("[AUTH] Fetching posts from page {}", page);
            self.client
                .get(&url)
                .query(&[("page", page), ("limit", 320)])
                .basic_auth(&self.auth.username, Some(&self.auth.api_key))
        } else {
            debug!("Fetching posts from page {}", page);
            self.client
                .get(&url)
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
