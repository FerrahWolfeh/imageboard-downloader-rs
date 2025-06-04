//! Post extractor for `https://danbooru.donmai.us`
//!
//! The danbooru extractor has the following features:
//! - Authentication
//! - Native blacklist (defined in user profile page)
//!
use self::models::DanbooruPost;

use crate::auth::{AuthState, ImageboardConfig};
use crate::extractor::caps::{Auth, ExtractorFeatures, SinglePostFetch};
use crate::extractor::Extractor;
use crate::extractor_config::{ServerConfig, DEFAULT_SERVERS};
use crate::{blacklist::BlacklistFilter, error::ExtractorError};
use ibdl_common::post::extension::Extension;
use ibdl_common::reqwest::Method;
use ibdl_common::serde_json;
use ibdl_common::tokio::time::{sleep, Instant};
use ibdl_common::{
    client, join_tags,
    log::debug,
    post::{rating::Rating, Post, PostQueue},
    reqwest::Client,
    ImageBoards,
};
use std::fmt::Display;
use std::time::Duration;

mod models;
mod pool;
mod unsync;

/// Main object to download posts
#[derive(Debug, Clone)]
pub struct DanbooruExtractor {
    client: Client,
    tags: Vec<String>,
    tag_string: String,
    auth_state: AuthState,
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
    server_cfg: ServerConfig,
}

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
        let config = DEFAULT_SERVERS.get("danbooru").unwrap().clone();

        // Use common client for all connections with a set User-Agent
        let client = client!(config);

        let mut strvec: Vec<String> = tags
            .iter()
            .map(|t| {
                let st: String = t.to_string();
                st
            })
            .collect();

        let extra_tags = if strvec.len() > 2 {
            strvec.split_off(1)
        } else {
            Vec::with_capacity(strvec.len().saturating_sub(2))
        };

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
            auth_state: AuthState::NotAuthenticated,
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
            server_cfg: config,
        }
    }

    fn new_with_config<S>(
        tags: &[S],
        download_ratings: &[Rating],
        disable_blacklist: bool,
        map_videos: bool,
        config: ServerConfig,
    ) -> Self
    where
        S: ToString + Display,
    {
        // Use common client for all connections with a set User-Agent
        let client = client!(config);

        let mut strvec: Vec<String> = tags
            .iter()
            .map(|t| {
                let st: String = t.to_string();
                st
            })
            .collect();

        let extra_tags = if strvec.len() > 2 {
            strvec.split_off(1)
        } else {
            Vec::with_capacity(strvec.len().saturating_sub(2))
        };

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
            auth_state: AuthState::NotAuthenticated,
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
            server_cfg: config,
        }
    }

    async fn search(&mut self, page: u16) -> Result<PostQueue, ExtractorError> {
        let mut posts = self.get_post_list(page, None).await?;

        if posts.is_empty() {
            return Err(ExtractorError::ZeroPosts);
        }

        posts.sort();
        posts.reverse();

        let qw = PostQueue {
            imageboard: self.server_cfg.server,
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
        let blacklist = BlacklistFilter::new(
            self.server_cfg.clone(),
            &self.excluded_tags,
            &self.download_ratings,
            self.disable_blacklist,
            !self.map_videos,
            self.selected_extension,
        )
        .await?;

        let mut fvec = limit.map_or_else(
            || Vec::with_capacity(self.server_cfg.max_post_limit as usize),
            |size| Vec::with_capacity(size as usize),
        );

        let mut page = 1;

        loop {
            let position = start_page.map_or(page, |n| page + n);

            debug!("Scanning page {}", position);

            let posts = self.get_post_list(position, limit).await?;
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
            imageboard: self.server_cfg.server,
            client: self.client.clone(),
            posts: fvec,
            tags: self.tags.clone(),
        };
        Ok(fin)
    }

    fn exclude_tags(&mut self, tags: &[String]) -> &mut Self {
        self.excluded_tags = tags.to_vec();
        self
    }

    fn force_extension(&mut self, extension: Extension) -> &mut Self {
        self.selected_extension = Some(extension);
        self
    }

    async fn get_post_list(
        &self,
        page: u16,
        limit: Option<u16>,
    ) -> Result<Vec<Post>, ExtractorError> {
        if self.server_cfg.post_list_url.is_none() {
            return Err(ExtractorError::UnsupportedOperation);
        }

        let mut request = self
            .client
            .request(Method::GET, self.server_cfg.post_list_url.as_ref().unwrap());

        // Fetch item list from page
        if self.auth_state.is_auth() {
            debug!("[AUTH] Fetching posts from page {}", page);
            request = request.basic_auth(&self.auth.username, Some(&self.auth.api_key));
        } else {
            debug!("Fetching posts from page {}", page);
        }

        let page_post_count = {
            limit.map_or(self.server_cfg.max_post_limit, |count| {
                if count < self.server_cfg.max_post_limit {
                    count
                } else {
                    self.server_cfg.max_post_limit
                }
            })
        };

        let req = request.query(&[
            ("page", &page.to_string()),
            ("limit", &page_post_count.to_string()),
            ("tags", &self.tag_string),
        ]);

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
                extension: Extension::guess_format(&c.file_ext.unwrap()),
                tags: tag_list,
                rating,
            }
        });

        Ok(mapper_iter.collect::<Vec<Post>>())
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

    fn features() -> ExtractorFeatures {
        ExtractorFeatures::from_bits_truncate(0b0001_1111) // AsyncFetch + TagSearch + SinglePostDownload + PoolDownload + Auth (Everything)
    }

    fn config(&self) -> ServerConfig {
        self.server_cfg.clone()
    }
}

impl Auth for DanbooruExtractor {
    async fn auth(&mut self, config: ImageboardConfig) -> Result<(), ExtractorError> {
        let mut cfg = config;

        self.excluded_tags
            .append(&mut cfg.user_data.blacklisted_tags);

        self.auth = cfg;
        self.auth_state = AuthState::Authenticated;
        Ok(())
    }
}

impl SinglePostFetch for DanbooruExtractor {
    fn map_post(&self, raw_json: String) -> Result<Post, ExtractorError> {
        let parsed_json: DanbooruPost = serde_json::from_str::<DanbooruPost>(raw_json.as_str())?;

        let tag_list = parsed_json.map_tags();

        let rt = parsed_json.rating.unwrap();
        let rating = if rt == "s" {
            Rating::Questionable
        } else {
            Rating::from_rating_str(&rt)
        };

        let post = Post {
            id: parsed_json.id.unwrap(),
            website: ImageBoards::Danbooru,
            md5: parsed_json.md5.unwrap(),
            url: parsed_json.file_url.unwrap(),
            extension: Extension::guess_format(&parsed_json.file_ext.unwrap()),
            tags: tag_list,
            rating,
        };

        Ok(post)
    }

    async fn get_post(&mut self, post_id: u32) -> Result<Post, ExtractorError> {
        if self.server_cfg.post_url.is_none() {
            return Err(ExtractorError::UnsupportedOperation);
        }

        let url = format!(
            "{}/{}.json",
            self.server_cfg.post_url.as_ref().unwrap(),
            post_id
        );

        // Fetch item list from page
        let req = if self.auth_state.is_auth() {
            debug!("[AUTH] Fetching post {}", post_id);
            self.client
                .get(url)
                .basic_auth(&self.auth.username, Some(&self.auth.api_key))
        } else {
            debug!("Fetching post {}", post_id);
            self.client.get(url)
        };

        let post_array = req.send().await?.text().await?;

        let start_point = Instant::now();

        let mtx = self.map_post(post_array)?;

        let end_iter = start_point.elapsed();

        debug!("Post mapping took {:?}", end_iter);
        Ok(mtx)
    }

    async fn get_posts(&mut self, posts: &[u32]) -> Result<Vec<Post>, ExtractorError> {
        let mut pvec = Vec::with_capacity(posts.len());

        for post_id in posts {
            let post = self.get_post(*post_id).await?;

            // This function is pretty heavy on API usage, so let's ease it up a little.
            debug!("Debouncing API calls by 500 ms");
            sleep(Duration::from_millis(500)).await;

            pvec.push(post);
        }
        Ok(pvec)
    }
}
