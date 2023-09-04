//! Post extractor for `https://danbooru.donmai.us`
//!
//! The danbooru extractor has the following features:
//! - Authentication
//! - Native blacklist (defined in user profile page)
//!
use self::models::DanbooruPost;

use super::{Auth, Extractor};
use crate::auth::ImageboardConfig;
use crate::{blacklist::BlacklistFilter, error::ExtractorError};
use async_trait::async_trait;
use ibdl_common::post::extension::Extension;
use ibdl_common::serde_json;
use ibdl_common::tokio::time::Instant;
use ibdl_common::{
    client, join_tags,
    log::debug,
    post::{rating::Rating, Post, PostQueue},
    reqwest::Client,
    ImageBoards,
};
use std::fmt::Display;

mod models;
mod pool;
mod unsync;

/// Main object to download posts
#[derive(Debug, Clone)]
pub struct DanbooruExtractor {
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
    extra_tags: Vec<String>,
    pool_id: Option<u32>,
    pool_last_items_first: bool,
}

#[async_trait]
impl Extractor for DanbooruExtractor {
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
        let client = client!(ImageBoards::Danbooru);

        let mut strvec: Vec<String> = tags
            .iter()
            .map(|t| {
                let st: String = t.to_string();
                st
            })
            .collect();

        let mut extra_tags = Vec::with_capacity(strvec.len().saturating_sub(2));

        if strvec.len() > 2 {
            let extra = strvec.split_off(1);
            extra_tags = extra;
        }

        debug!("Tag List: {:?}", strvec);
        if !extra_tags.is_empty() {
            debug!("Extra tags: {:?}", extra_tags);
        }

        // Merge all tags in the URL format
        let tag_string = join_tags!(strvec);

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
            extra_tags,
            pool_id: None,
            pool_last_items_first: false,
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
            imageboard: ImageBoards::Danbooru,
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
            ImageBoards::Danbooru,
            &self.excluded_tags,
            &self.download_ratings,
            self.disable_blacklist,
            !self.map_videos,
            self.selected_extension,
        )
        .await?;

        let mut fvec = if let Some(size) = limit {
            Vec::with_capacity(size as usize)
        } else {
            Vec::with_capacity(200)
        };

        let mut page = 1;

        loop {
            let position = if let Some(n) = start_page {
                page + n
            } else {
                page
            };

            debug!("Scanning page {}", position);

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

            if page == 100 {
                break;
            }

            page += 1;
        }

        if fvec.is_empty() {
            return Err(ExtractorError::ZeroPosts);
        }

        fvec.sort();
        fvec.reverse();

        let fin = PostQueue {
            imageboard: ImageBoards::Danbooru,
            client: self.client.clone(),
            posts: fvec,
            tags: self.tags.clone(),
        };
        Ok(fin)
    }

    async fn get_post_list(&self, page: u16) -> Result<Vec<Post>, ExtractorError> {
        let url = format!(
            "{}?tags={}",
            ImageBoards::Danbooru.post_url(),
            &self.tag_string
        );

        // Fetch item list from page
        let req = if self.auth_state {
            debug!("[AUTH] Fetching posts from page {}", page);
            self.client
                .get(url)
                .query(&[("page", page), ("limit", 200)])
                .basic_auth(&self.auth.username, Some(&self.auth.api_key))
        } else {
            debug!("Fetching posts from page {}", page);
            self.client
                .get(url)
                .query(&[("page", page), ("limit", 200)])
        };

        let post_array = req.send().await?.text().await?;

        let start_point = Instant::now();

        let mtx = self.map_posts(post_array)?;

        let end_iter = start_point.elapsed();

        debug!("List size: {}", mtx.len());
        debug!("Post mapping took {:?}", end_iter);
        Ok(mtx)
    }

    fn map_posts(&self, raw_json: String) -> Result<Vec<Post>, ExtractorError> {
        let parsed_json: Vec<DanbooruPost> =
            serde_json::from_str::<Vec<DanbooruPost>>(raw_json.as_str())?;

        let batch = parsed_json.into_iter().filter(|c| c.file_url.is_some());

        let mapper_iter = batch.map(|c| {
            let tag_list = c.map_tags();

            let rt = c.rating.unwrap();
            let rating = if rt == "s" {
                Rating::Questionable
            } else {
                Rating::from_rating_str(&rt)
            };

            Post {
                id: c.id.unwrap(),
                website: ImageBoards::Danbooru,
                md5: c.md5.unwrap(),
                url: c.file_url.unwrap(),
                extension: c.file_ext.unwrap(),
                tags: tag_list,
                rating,
            }
        });

        Ok(Vec::from_iter(mapper_iter))
    }

    fn client(&self) -> Client {
        self.client.clone()
    }

    fn total_removed(&self) -> u64 {
        self.total_removed
    }

    fn imageboard(&self) -> ImageBoards {
        ImageBoards::Danbooru
    }

    fn exclude_tags(&mut self, tags: &[String]) -> &mut Self {
        self.excluded_tags = tags.to_vec();
        self
    }
}

#[async_trait]
impl Auth for DanbooruExtractor {
    async fn auth(&mut self, config: ImageboardConfig) -> Result<(), ExtractorError> {
        let mut cfg = config;

        self.excluded_tags
            .append(&mut cfg.user_data.blacklisted_tags);

        self.auth = cfg;
        self.auth_state = true;
        Ok(())
    }
}
