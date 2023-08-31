//! Post extractor for `https://e621.net`
//!
//! The e621 extractor has the following features:
//! - Authentication
//! - Native blacklist (defined in user profile page)
//!
use crate::auth::ImageboardConfig;
use async_trait::async_trait;
use ibdl_common::post::extension::Extension;
use ibdl_common::reqwest::Client;
use ibdl_common::serde_json;
use ibdl_common::{
    client, join_tags,
    log::debug,
    post::{rating::Rating, Post, PostQueue},
    tokio, ImageBoards,
};
use std::fmt::Display;
use std::time::Duration;
use tokio::time::{sleep, Instant};

use crate::{
    blacklist::BlacklistFilter, error::ExtractorError, websites::e621::models::E621TopLevel,
};

use super::{Auth, Extractor};

mod models;
mod pool;
mod unsync;

//const _E621_FAVORITES: &str = "https://e621.net/favorites.json";

/// Main object to download posts
#[derive(Clone, Debug)]
pub struct E621Extractor {
    client: Client,
    tags: Vec<String>,
    tag_string: String,
    auth_state: bool,
    auth: ImageboardConfig,
    download_ratings: Vec<Rating>,
    disable_blacklist: bool,
    total_removed: u64,
    map_videos: bool,
    excluded_tags: Vec<String>,
    selected_extension: Option<Extension>,
    pool_id: Option<u32>,
}

#[async_trait]
impl Extractor for E621Extractor {
    fn new<S>(
        tags: &[S],
        download_ratings: &[Rating],
        disable_blacklist: bool,
        map_videos: bool,
    ) -> Self
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
            map_videos,
            excluded_tags: vec![],
            selected_extension: None,
            pool_id: None,
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

    fn force_extension(&mut self, extension: Extension) -> &mut Self {
        self.selected_extension = Some(extension);
        self
    }

    async fn full_search(
        &mut self,
        start_page: Option<u16>,
        limit: Option<u16>,
    ) -> Result<PostQueue, ExtractorError> {
        let blacklist = BlacklistFilter::new(
            ImageBoards::E621,
            &self.auth.user_data.blacklisted_tags,
            &self.download_ratings,
            self.disable_blacklist,
            !self.map_videos,
            self.selected_extension,
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

            let posts = self.get_post_list(position).await?;
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

        let items = req.send().await?.text().await?;

        let start_point = Instant::now();

        let pl = self.map_posts(items)?;

        let end_point = Instant::now();

        debug!("List size: {}", pl.len());
        debug!("Post mapping took {:?}", end_point - start_point);
        Ok(pl)
    }

    fn map_posts(&self, raw_json: String) -> Result<Vec<Post>, ExtractorError> {
        let mut items: E621TopLevel = serde_json::from_str(raw_json.as_str()).unwrap();

        let post_iter = items.posts.iter_mut().filter(|c| c.file.url.is_some());

        let mut post_list: Vec<Post> = Vec::with_capacity(post_iter.size_hint().0);

        post_iter.for_each(|c| {
            let tag_list = c.tags.map_tags();

            let unit = Post {
                id: c.id.unwrap(),
                website: ImageBoards::E621,
                url: c.file.url.clone().unwrap(),
                md5: c.file.md5.clone().unwrap(),
                extension: c.file.ext.clone().unwrap(),
                tags: tag_list,
                rating: Rating::from_rating_str(&c.rating),
            };

            post_list.push(unit);
        });

        Ok(post_list)
    }

    fn client(&self) -> Client {
        self.client.clone()
    }

    fn total_removed(&self) -> u64 {
        self.total_removed
    }

    fn imageboard(&self) -> ImageBoards {
        ImageBoards::E621
    }

    fn exclude_tags(&mut self, tags: &[String]) -> &mut Self {
        self.excluded_tags = tags.to_vec();
        self
    }
}

#[async_trait]
impl Auth for E621Extractor {
    async fn auth(&mut self, config: ImageboardConfig) -> Result<(), ExtractorError> {
        let mut cfg = config;

        self.excluded_tags
            .append(&mut cfg.user_data.blacklisted_tags);

        self.auth = cfg;
        self.auth_state = true;

        Ok(())
    }
}
