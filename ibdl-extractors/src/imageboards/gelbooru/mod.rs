//! Post extractor for Gelbooru-based imageboards
//!
//! This extractor is compatible with these imageboards:
//! * `Imageboards::Rule34`
//! * `Imageboards::Realbooru`
//! * `Imageboards::Gelbooru`
//!
//!

// NOTE: https://gelbooru.com/index.php?page=dapi&s=tag&q=index&json=1&names=folinic_(arknights)%20arknights%20ru_zhai%20highres%20black_hair
// This is to search all tags and their meanings.
// I've to do an enum based on this thing.

use ibdl_common::post::extension::Extension;
use ibdl_common::reqwest::Client;
use ibdl_common::serde_json::{self};
use ibdl_common::tokio::time::{sleep, Instant};
use ibdl_common::{
    extract_ext_from_url,
    log::debug,
    post::{rating::Rating, Post, PostQueue},
    ImageBoards,
};
use std::fmt::Display;
use std::time::Duration;

use crate::extractor::caps::ExtractorFeatures;
use crate::extractor::common::convert_tags_to_string;
use crate::extractor::Extractor;
use crate::extractor_config::{ServerConfig, DEFAULT_SERVERS};
use crate::imageboards::gelbooru::models::GelbooruTopLevel;
use crate::prelude::SinglePostFetch;
use crate::{blacklist::BlacklistFilter, error::ExtractorError};

mod gelbooru_old;
mod models;
mod unsync;

pub struct GelbooruExtractor {
    client: Client,
    tags: Vec<String>,
    tag_string: String,
    disable_blacklist: bool,
    total_removed: u64,
    download_ratings: Vec<Rating>,
    map_videos: bool,
    excluded_tags: Vec<String>,
    selected_extension: Option<Extension>,
    server_cfg: ServerConfig,
    // auth: ImageboardConfig,
    // auth_state: AuthState
}

impl Extractor for GelbooruExtractor {
    fn new<S>(
        tags: &[S],
        download_ratings: &[Rating],
        disable_blacklist: bool,
        map_videos: bool,
    ) -> Self
    where
        S: ToString + Display,
    {
        let config = DEFAULT_SERVERS.get("gelbooru").unwrap().clone();

        // Use common client for all connections with a set User-Agent
        let client = Client::builder()
            .user_agent(&config.client_user_agent)
            .build()
            .unwrap();

        let (string_vec, tag_string) = convert_tags_to_string(tags);

        Self {
            client,
            tags: string_vec,
            tag_string,
            disable_blacklist,
            total_removed: 0,
            download_ratings: download_ratings.to_vec(),
            map_videos,
            excluded_tags: vec![],
            selected_extension: None,
            server_cfg: config,
            // auth_state: AuthState::NotAuthenticated,
            // auth: ImageboardConfig::default()
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
        let client = Client::builder()
            .user_agent(&config.client_user_agent)
            .build()
            .unwrap();

        let (strvec, tag_string) = convert_tags_to_string(tags);

        Self {
            client,
            tags: strvec,
            tag_string,
            disable_blacklist,
            total_removed: 0,
            download_ratings: download_ratings.to_vec(),
            map_videos,
            excluded_tags: vec![],
            selected_extension: None,
            server_cfg: config,
            // auth_state: AuthState::NotAuthenticated,
            // auth: ImageboardConfig::default()
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
            imageboard: ImageBoards::Gelbooru,
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
            &Vec::default(),
            &self.download_ratings,
            self.disable_blacklist,
            !self.map_videos,
            self.selected_extension,
        )
        .await?;

        let mut fvec = if let Some(size) = limit {
            Vec::with_capacity(size as usize)
        } else {
            Vec::with_capacity(self.server_cfg.max_post_limit as usize)
        };

        let mut page = 1;

        loop {
            let position = start_page.map_or(page - 1, |n| page + n - 1);

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

            if size < self.server_cfg.max_post_limit as usize || page == 100 {
                break;
            }

            page += 1;

            //debounce
            debug!("Debouncing API calls by 500 ms");
            sleep(Duration::from_millis(500)).await;
        }

        if fvec.is_empty() {
            return Err(ExtractorError::ZeroPosts);
        }

        fvec.sort();
        fvec.reverse();

        let fin = PostQueue {
            imageboard: ImageBoards::Gelbooru,
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
        };

        let page_post_count = {
            limit.map_or(self.server_cfg.max_post_limit, |count| {
                if count < self.server_cfg.max_post_limit {
                    count
                } else {
                    self.server_cfg.max_post_limit
                }
            })
        };

        let items = self
            .client
            .get(self.server_cfg.post_list_url.as_ref().unwrap())
            .query(&[
                ("tags", &self.tag_string),
                ("pid", &page.to_string()),
                ("limit", &page_post_count.to_string()),
            ])
            .send()
            .await?
            .text()
            .await?;

        #[cfg(debug_assertions)]
        debug!("{}", items);

        self.map_posts(items)
    }

    fn map_posts(&self, raw_json: String) -> Result<Vec<Post>, ExtractorError> {
        let parsed_json: GelbooruTopLevel =
            serde_json::from_str::<GelbooruTopLevel>(raw_json.as_str())?;

        let batch = parsed_json
            .post
            .into_iter()
            .filter(|c| c.file_url.is_some());

        let mapper_iter = batch.map(|c| {
            let tag_list = c.map_tags();

            let rt = c.rating.unwrap();
            let rating = Rating::from_rating_str(&rt);
            let xt = c.file_url.unwrap();

            let extension = extract_ext_from_url!(xt);

            Post {
                id: c.id.unwrap(),
                website: ImageBoards::Danbooru,
                md5: c.md5.unwrap(),
                url: xt,
                extension: Extension::guess_format(&extension),
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
        ImageBoards::Gelbooru
    }

    fn features() -> ExtractorFeatures {
        ExtractorFeatures::from_bits_truncate(0b0000_0111) // AsyncFetch + TagSearch + SinglePostFetch
    }

    fn config(&self) -> ServerConfig {
        self.server_cfg.clone()
    }
}

// impl Auth for GelbooruExtractor {
//     async fn auth(&mut self, config: ImageboardConfig) -> Result<(), ExtractorError> {
//         let mut cfg = config;
//
//         self.excluded_tags
//             .append(&mut cfg.user_data.blacklisted_tags);
//
//         self.auth = cfg;
//         self.auth_state = AuthState::Authenticated;
//         Ok(())
//     }
// }

impl SinglePostFetch for GelbooruExtractor {
    fn map_post(&self, _raw_json: String) -> Result<Post, ExtractorError> {
        unimplemented!("Unsupported operation! Use `self.map_posts()` instead.");
    }

    async fn get_post(&mut self, post_id: u32) -> Result<Post, ExtractorError> {
        if self.server_cfg.post_url.is_none() {
            return Err(ExtractorError::UnsupportedOperation);
        };

        let url = format!(
            "{}/{}.json",
            self.server_cfg.post_url.as_ref().unwrap(),
            post_id
        );

        let items = self.client.get(&url).send().await?.text().await?;

        let start_point = Instant::now();

        let mtx = self.map_posts(items)?;

        mtx.first().map_or_else(
            || Err(ExtractorError::ZeroPosts),
            |post| {
                let end_iter = start_point.elapsed();

                debug!("Post mapping took {:?}", end_iter);
                Ok(post.clone())
            },
        )
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
