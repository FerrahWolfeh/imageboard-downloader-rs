//! Post extractor for `https://e621.net`
//!
//! The e621 extractor has the following features:
//! - Authentication
//! - Native blacklist (defined in user profile page)
//!
use async_trait::async_trait;
use ibdl_common::reqwest::Client;
use ibdl_common::{
    auth::{auth_prompt, ImageboardConfig},
    client, join_tags,
    log::debug,
    post::{rating::Rating, Post, PostQueue},
    tokio, ImageBoards,
};
use rayon::prelude::*;
use std::fmt::Display;
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::{sleep, Instant};

use crate::{
    blacklist::BlacklistFilter, error::ExtractorError, websites::e621::models::E621TopLevel,
};

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
    download_ratings: Vec<Rating>,
    disable_blacklist: bool,
    total_removed: u64,
}

#[async_trait]
impl Extractor for E621Extractor {
    fn new<S>(tags: &[S], download_ratings: &[Rating], disable_blacklist: bool) -> Self
    where
        S: ToString + Display,
    {
        // Use common client for all connections with a set User-Agent
        let client = client!(ImageBoards::E621);

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
            download_ratings: download_ratings.to_vec(),
            disable_blacklist,
            total_removed: 0,
        }
    }

    async fn search(&mut self, page: u16) -> Result<PostQueue, ExtractorError> {
        let mut posts = Self::get_post_list(self, page).await?;

        if posts.is_empty() {
            return Err(ExtractorError::ZeroPosts);
        }

        posts.sort();
        posts.reverse();

        let qw = PostQueue {
            imageboard: ImageBoards::E621,
            client: self.client.clone(),
            posts,
            tags: self.tags.clone(),
        };

        Ok(qw)
    }

    async fn full_search(
        &mut self,
        start_page: Option<u16>,
        limit: Option<u16>,
    ) -> Result<PostQueue, ExtractorError> {
        let blacklist = BlacklistFilter::init(
            ImageBoards::E621,
            &self.auth.user_data.blacklisted_tags,
            &self.download_ratings,
            self.disable_blacklist,
        )
        .await?;

        let mut fvec = if let Some(size) = limit {
            Vec::with_capacity(size as usize)
        } else {
            Vec::with_capacity(320)
        };

        let mut page = 1;

        loop {
            let position = if let Some(n) = start_page {
                page + n
            } else {
                page
            };

            let posts = Self::get_post_list(self, position).await?;
            let size = posts.len();

            if size == 0 {
                break;
            }

            let mut list = if !self.disable_blacklist || !self.download_ratings.is_empty() {
                let (removed, posts) = blacklist.filter(posts);
                self.total_removed += removed;
                posts
            } else {
                posts
            };

            fvec.append(&mut list);

            if let Some(num) = limit {
                if fvec.len() >= num as usize {
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

        if fvec.is_empty() {
            return Err(ExtractorError::ZeroPosts);
        }

        fvec.sort();
        fvec.reverse();

        let fin = PostQueue {
            imageboard: ImageBoards::E621,
            client: self.client.clone(),
            posts: fvec,
            tags: self.tags.clone(),
        };

        Ok(fin)
    }

    async fn get_post_list(&self, page: u16) -> Result<Vec<Post>, ExtractorError> {
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
        let post_iter = items.posts.into_iter().filter(|c| c.file.url.is_some());

        let post_list: Mutex<Vec<Post>> = Mutex::new(Vec::with_capacity(post_iter.size_hint().0));

        post_iter.par_bridge().for_each(|c| {
            let full_size = c.tags.artist.len()
                + c.tags.character.len()
                + c.tags.general.len()
                + c.tags.copyright.len()
                + c.tags.lore.len()
                + c.tags.meta.len()
                + c.tags.species.len();

            let mut tag_list = Vec::with_capacity(full_size);
            tag_list.extend(c.tags.character.into_iter());
            tag_list.extend(c.tags.artist.into_iter());
            tag_list.extend(c.tags.general.into_iter());
            tag_list.extend(c.tags.copyright.into_iter());
            tag_list.extend(c.tags.lore.into_iter());
            tag_list.extend(c.tags.meta.into_iter());
            tag_list.extend(c.tags.species.into_iter());

            let unit = Post {
                id: c.id.unwrap(),
                url: c.file.url.clone().unwrap(),
                md5: c.file.md5.clone().unwrap(),
                extension: c.file.ext.unwrap(),
                tags: tag_list,
                rating: Rating::from_rating_str(&c.rating),
            };

            post_list.lock().unwrap().push(unit);
        });

        let end_point = Instant::now();

        let pl = post_list.lock().unwrap().clone();
        drop(post_list);

        debug!("List size: {}", pl.len());
        debug!("Post mapping took {:?}", end_point - start_point);
        Ok(pl)
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
