//! Post extractor for Gelbooru-based imageboards
//!
//! This extractor is compatible with these imageboards:
//! * `Imageboards::Rule34`
//! * `Imageboards::Realbooru`
//! * `Imageboards::Gelbooru`
//!

use async_trait::async_trait;
use ibdl_common::reqwest::Client;
use ibdl_common::serde_json::{self, Value};
use ibdl_common::tokio::time::{sleep, Instant};
use ibdl_common::{
    client, extract_ext_from_url, join_tags,
    log::debug,
    post::{rating::Rating, Post, PostQueue},
    ImageBoards,
};
use std::fmt::Display;
use std::sync::Mutex;
use std::time::Duration;

use crate::{blacklist::BlacklistFilter, error::ExtractorError};

use super::{Extractor, MultiWebsite};

pub struct GelbooruExtractor {
    active_imageboard: ImageBoards,
    client: Client,
    tags: Vec<String>,
    tag_string: String,
    disable_blacklist: bool,
    total_removed: u64,
    download_ratings: Vec<Rating>,
}

#[async_trait]
impl Extractor for GelbooruExtractor {
    #[allow(unused_variables)]
    fn new<S>(tags: &[S], download_ratings: &[Rating], disable_blacklist: bool) -> Self
    where
        S: ToString + Display,
    {
        // Use common client for all connections with a set User-Agent
        let client = Client::builder()
            .user_agent(ImageBoards::Rule34.user_agent())
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
        let blacklist = BlacklistFilter::init(
            self.active_imageboard,
            &Vec::default(),
            &self.download_ratings,
            self.disable_blacklist,
        )
        .await?;

        let mut fvec = if let Some(size) = limit {
            Vec::with_capacity(size as usize)
        } else {
            Vec::with_capacity(self.active_imageboard.max_post_limit())
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

            if size < self.active_imageboard.max_post_limit() || page == 100 {
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

    async fn get_post_list(&self, page: u16) -> Result<Vec<Post>, ExtractorError> {
        let url = format!(
            "{}&tags={}",
            self.active_imageboard.post_url(),
            self.tag_string
        );

        let items = self
            .client
            .get(&url)
            .query(&[("pid", page), ("limit", 1000)])
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
            let posts = Self::gelbooru_new_path(it);

            return Ok(posts);
        }
        Err(ExtractorError::PostMapFailure)
    }

    fn client(self) -> Client {
        self.client
    }

    fn total_removed(&self) -> u64 {
        self.total_removed
    }
}

impl MultiWebsite for GelbooruExtractor {
    /// Sets the imageboard to extract posts from
    ///
    /// If not set, defaults to `ImageBoards::Rule34`
    fn set_imageboard(self, imageboard: ImageBoards) -> Result<Self, ExtractorError>
    where
        Self: std::marker::Sized,
    {
        let imageboard = match imageboard {
            ImageBoards::Gelbooru | ImageBoards::Realbooru | ImageBoards::Rule34 => imageboard,
            _ => {
                return Err(ExtractorError::InvalidImageboard {
                    imgboard: imageboard.to_string(),
                })
            }
        };
        let client = client!(imageboard);

        Ok(Self {
            active_imageboard: imageboard,
            client,
            tags: self.tags,
            tag_string: self.tag_string,
            disable_blacklist: self.disable_blacklist,
            total_removed: self.total_removed,
            download_ratings: self.download_ratings,
        })
    }
}

impl GelbooruExtractor {
    fn gelbooru_old_path(&self, list: &[Value]) -> Vec<Post> {
        let start = Instant::now();
        let post_iter = list.iter().filter(|f| f["hash"].as_str().is_some());

        let post_mtx: Mutex<Vec<Post>> = Mutex::new(Vec::with_capacity(post_iter.size_hint().0));

        post_iter.for_each(|f| {
            let tag_iter = f["tags"].as_str().unwrap().split(' ');

            let mut tags = Vec::with_capacity(tag_iter.size_hint().0);

            tag_iter.for_each(|f| {
                tags.push(f.to_string());
            });

            let rating = Rating::from_rating_str(f["rating"].as_str().unwrap());

            let file = f["image"].as_str().unwrap();

            let md5 = f["hash"].as_str().unwrap().to_string();

            let ext = extract_ext_from_url!(file);

            let drop_url = if self.active_imageboard == ImageBoards::Realbooru {
                format!(
                    "https://realbooru.com/images/{}/{}.{}",
                    f["directory"].as_str().unwrap(),
                    &md5,
                    &ext
                )
            } else {
                f["file_url"].as_str().unwrap().to_string()
            };

            let unit = Post {
                id: f["id"].as_u64().unwrap(),
                url: drop_url,
                md5,
                extension: extract_ext_from_url!(file),
                rating,
                tags,
            };

            post_mtx.lock().unwrap().push(unit);
        });

        let end = Instant::now();

        let posts = post_mtx.lock().unwrap().clone();
        drop(post_mtx);

        debug!("List size: {}", posts.len());
        debug!("Post mapping took {:?}", end - start);

        posts
    }

    fn gelbooru_new_path(list: &[Value]) -> Vec<Post> {
        let start = Instant::now();
        let post_iter = list.iter().filter(|i| i["file_url"].as_str().is_some());

        let post_mtx: Mutex<Vec<Post>> = Mutex::new(Vec::with_capacity(post_iter.size_hint().0));

        post_iter.for_each(|post| {
            let url = post["file_url"].as_str().unwrap().to_string();
            let tag_iter = post["tags"].as_str().unwrap().split(' ');

            let mut tags = Vec::with_capacity(tag_iter.size_hint().0);

            tag_iter.for_each(|i| {
                tags.push(i.to_string());
            });

            let unit = Post {
                id: post["id"].as_u64().unwrap(),
                md5: post["md5"].as_str().unwrap().to_string(),
                url: url.clone(),
                extension: extract_ext_from_url!(url),
                tags,
                rating: Rating::from_rating_str(post["rating"].as_str().unwrap()),
            };

            post_mtx.lock().unwrap().push(unit);
        });

        let end = Instant::now();

        let posts = post_mtx.lock().unwrap().clone();
        drop(post_mtx);

        debug!("List size: {}", posts.len());
        debug!("Post mapping took {:?}", end - start);

        posts
    }
}
