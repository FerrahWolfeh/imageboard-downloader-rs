//! Post extractor for Gelbooru-based imageboards
//!
//! This extractor is compatible with these imageboards:
//! * `Imageboards::Rule34`
//! * `Imageboards::Realbooru`
//! * `Imageboards::Gelbooru`
//!
//!

//NOTE: https://gelbooru.com/index.php?page=dapi&s=tag&q=index&json=1&names=folinic_(arknights)%20arknights%20ru_zhai%20highres%20black_hair
// This is to search all tags and their meanings.
// Gotta do an enum based on this thing.

use async_trait::async_trait;
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

use crate::extractor_config::DEFAULT_SERVERS;
use crate::{blacklist::BlacklistFilter, error::ExtractorError};

use super::{Extractor, ServerConfig, SinglePostFetch};

mod unsync;

pub struct GelbooruExtractor {
    active_imageboard: ImageBoards,
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

#[async_trait]
impl Extractor for GelbooruExtractor {
    #[allow(unused_variables)]
    fn new<S>(
        tags: &[S],
        download_ratings: &[Rating],
        disable_blacklist: bool,
        map_videos: bool,
    ) -> Self
    where
        S: ToString + Display,
    {
        let config = DEFAULT_SERVERS.get("rule34").unwrap().clone();

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
            active_imageboard: ImageBoards::Rule34,
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

    #[allow(unused_variables)]
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
            active_imageboard: config.server,
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
        let mut posts = Self::get_post_list(self, page).await?;

        if posts.is_empty() {
            return Err(ExtractorError::ZeroPosts);
        }

        posts.sort();
        posts.reverse();

        let qw = PostQueue {
            imageboard: self.active_imageboard,
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
            self.active_imageboard,
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
            Vec::with_capacity(self.server_cfg.max_post_limit)
        };

        let mut page = 1;

        loop {
            let position = if let Some(n) = start_page {
                page + n - 1
            } else {
                page - 1
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

            if size < self.server_cfg.max_post_limit || page == 100 {
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
            imageboard: self.active_imageboard,
            client: self.client.clone(),
            posts: fvec,
            tags: self.tags.clone(),
        };

        Ok(fin)
    }

    fn force_extension(&mut self, extension: Extension) -> &mut Self {
        self.selected_extension = Some(extension);
        self
    }

    async fn get_post_list(&self, page: u16) -> Result<Vec<Post>, ExtractorError> {
        if self.server_cfg.post_list_url.is_none() {
            return Err(ExtractorError::UnsupportedOperation);
        };

        let items = self
            .client
            .get(self.server_cfg.post_list_url.as_ref().unwrap())
            .query(&[
                ("tags", &self.tag_string),
                ("pid", &page.to_string()),
                ("limit", &1000.to_string()),
            ])
            .send()
            .await?
            .text()
            .await?;

        Ok(self.map_posts(items)?)
    }

    fn map_posts(&self, raw_json: String) -> Result<Vec<Post>, ExtractorError> {
        let items = serde_json::from_str::<Value>(raw_json.as_str()).unwrap();

        if let Some(arr) = items.as_array() {
            let posts = self.gelbooru_old_path(arr);

            return Ok(posts);
        }

        if let Some(it) = items["post"].as_array() {
            let posts = self.gelbooru_new_path(it);

            return Ok(posts);
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
        self.active_imageboard
    }

    fn exclude_tags(&mut self, tags: &[String]) -> &mut Self {
        self.excluded_tags = tags.to_vec();
        self
    }
}

// impl MultiWebsite for GelbooruExtractor {
//     /// Sets the imageboard to extract posts from
//     ///
//     /// If not set, defaults to `ImageBoards::Rule34`
//     fn set_imageboard(&mut self, imageboard: ImageBoards) -> &mut Self
//     where
//         Self: std::marker::Sized,
//     {
//         self.active_imageboard = match imageboard {
//             ImageBoards::Gelbooru | ImageBoards::Realbooru | ImageBoards::Rule34 => imageboard,
//             _ => ImageBoards::Gelbooru,
//         };

//         self
//     }
// }

impl GelbooruExtractor {
    fn gelbooru_old_path(&self, list: &[Value]) -> Vec<Post> {
        let start = Instant::now();
        let post_iter = list.iter().filter(|f| f["hash"].as_str().is_some());

        let mut post_mtx: Vec<Post> = Vec::with_capacity(post_iter.size_hint().0);

        post_iter.for_each(|f| {
            let unit = self.gelbooru_old_path_map_post(f);

            post_mtx.push(unit);
        });

        let end = Instant::now();

        debug!("List size: {}", post_mtx.len());
        debug!("Post mapping took {:?}", end - start);

        post_mtx
    }

    fn gelbooru_new_path(&self, list: &[Value]) -> Vec<Post> {
        let start = Instant::now();
        let post_iter = list.iter().filter(|i| i["file_url"].as_str().is_some());

        let mut post_mtx: Vec<Post> = Vec::with_capacity(post_iter.size_hint().0);

        post_iter.for_each(|post| {
            let unit = self.gelbooru_new_path_map_post(post);

            post_mtx.push(unit);
        });

        let end = Instant::now();

        debug!("List size: {}", post_mtx.len());
        debug!("Post mapping took {:?}", end - start);

        post_mtx
    }

    #[inline]
    fn gelbooru_new_path_map_post(&self, post: &Value) -> Post {
        let url = post["file_url"].as_str().unwrap().to_string();
        let tag_iter = post["tags"].as_str().unwrap().split(' ');

        let mut tags = Vec::with_capacity(tag_iter.size_hint().0);

        tag_iter.for_each(|i| {
            tags.push(Tag::new(i, TagType::Any));
        });

        let extension = extract_ext_from_url!(url);

        Post {
            id: post["id"].as_u64().unwrap(),
            website: self.active_imageboard,
            md5: post["md5"].as_str().unwrap().to_string(),
            url,
            extension,
            tags,
            rating: Rating::from_rating_str(post["rating"].as_str().unwrap()),
        }
    }

    #[inline]
    fn gelbooru_old_path_map_post(&self, post: &Value) -> Post {
        let tag_iter = post["tags"].as_str().unwrap().split(' ');

        let mut tags = Vec::with_capacity(tag_iter.size_hint().0);

        tag_iter.for_each(|f| {
            tags.push(Tag::new(f, TagType::Any));
        });

        let rating = Rating::from_rating_str(post["rating"].as_str().unwrap());

        let file = post["image"].as_str().unwrap();

        let md5 = post["hash"].as_str().unwrap().to_string();

        let ext = extract_ext_from_url!(file);

        let drop_url = if self.active_imageboard == ImageBoards::Realbooru {
            format!(
                "https://realbooru.com/images/{}/{}",
                post["directory"].as_str().unwrap(),
                &file,
            )
        } else {
            post["file_url"].as_str().unwrap().to_string()
        };

        Post {
            id: post["id"].as_u64().unwrap(),
            website: self.active_imageboard,
            url: drop_url,
            md5,
            extension: ext,
            rating,
            tags,
        }
    }
}

#[async_trait]
impl SinglePostFetch for GelbooruExtractor {
    fn map_post(&self, _raw_json: String) -> Result<Post, ExtractorError> {
        unimplemented!();
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

        if let Some(post) = mtx.get(0) {
            let end_iter = start_point.elapsed();

            debug!("Post mapping took {:?}", end_iter);
            Ok(post.clone())
        } else {
            Err(ExtractorError::ZeroPosts)
        }
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
