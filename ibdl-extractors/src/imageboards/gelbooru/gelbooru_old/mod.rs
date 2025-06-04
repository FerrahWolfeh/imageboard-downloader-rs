//! Post extractor for Gelbooru-based imageboards
//!
//! This extractor is compatible with these imageboards:
//! * `Imageboards::Realbooru`
//!

use ibdl_common::post::extension::Extension;
use ibdl_common::post::tags::{Tag, TagType};
use ibdl_common::reqwest::Client;
use ibdl_common::serde_json::{self, Value};
use ibdl_common::tokio::time::{sleep, Instant};
use ibdl_common::{
    extract_ext_from_url, join_tags,
    log::debug,
    post::{rating::Rating, Post, PostQueue},
    ImageBoards,
};
use std::fmt::Display;
use std::time::Duration;

use crate::extractor::caps::ExtractorFeatures;
use crate::extractor::Extractor;
use crate::extractor_config::{ServerConfig, DEFAULT_SERVERS};
use crate::{blacklist::BlacklistFilter, error::ExtractorError};

mod unsync;

pub struct GelbooruV0_2Extractor {
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
}

impl Extractor for GelbooruV0_2Extractor {
    fn new<S>(
        tags: &[S],
        download_ratings: &[Rating],
        disable_blacklist: bool,
        map_videos: bool,
    ) -> Self
    where
        S: ToString + Display,
    {
        let config = DEFAULT_SERVERS.get("realbooru").unwrap().clone();

        // Use common client for all connections with a set User-Agent
        let client = Client::builder()
            .user_agent(&config.client_user_agent)
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
        debug!("Tag List: {}", tag_string);

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
            disable_blacklist,
            total_removed: 0,
            download_ratings: download_ratings.to_vec(),
            map_videos,
            excluded_tags: vec![],
            selected_extension: None,
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
            imageboard: ImageBoards::GelbooruV0_2,
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
            imageboard: ImageBoards::GelbooruV0_2,
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
        let items = serde_json::from_str::<Value>(raw_json.as_str())?;

        if let Some(arr) = items.as_array() {
            let start = Instant::now();
            let post_iter = arr.iter().filter(|f| f["hash"].as_str().is_some());

            let mut post_mtx: Vec<Post> = Vec::with_capacity(post_iter.size_hint().0);

            post_iter.for_each(|post| {
                let tag_iter = post["tags"].as_str().unwrap().split(' ');

                let mut tags = Vec::with_capacity(tag_iter.size_hint().0);

                tag_iter.for_each(|f| {
                    tags.push(Tag::new(f, TagType::Any));
                });

                let rating = Rating::from_rating_str(post["rating"].as_str().unwrap());

                let file = post["image"].as_str().unwrap();

                let md5 = post["hash"].as_str().unwrap().to_string();

                let ext = extract_ext_from_url!(file);

                let imgu = self
                    .server_cfg
                    .image_url
                    .as_ref()
                    .map_or(&self.server_cfg.base_url, |img_url| img_url);

                let drop_url = format!(
                    "{}/images/{}/{}.{}",
                    imgu,
                    post["directory"].as_str().unwrap(),
                    &md5,
                    ext
                );

                let unit = Post {
                    id: post["id"].as_u64().unwrap(),
                    website: ImageBoards::GelbooruV0_2,
                    url: drop_url,
                    md5,
                    extension: Extension::guess_format(&ext),
                    rating,
                    tags,
                };

                post_mtx.push(unit);
            });

            let end = Instant::now();

            debug!("List size: {}", post_mtx.len());
            debug!("Post mapping took {:?}", end - start);

            return Ok(post_mtx);
        }

        Err(ExtractorError::PostMapFailure)
    }

    fn client(&self) -> Client {
        self.client.clone()
    }

    fn total_removed(&self) -> u64 {
        self.total_removed
    }

    fn imageboard(&self) -> ImageBoards {
        ImageBoards::GelbooruV0_2
    }

    fn features() -> ExtractorFeatures {
        ExtractorFeatures::from_bits_truncate(0b0000_0111) // AsyncFetch + TagSearch + SinglePostFetch
    }

    fn config(&self) -> ServerConfig {
        self.server_cfg.clone()
    }
}
