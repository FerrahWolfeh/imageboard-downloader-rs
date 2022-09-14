//! Post extractor for `https://konachan.com` and other Moebooru imageboards
use async_trait::async_trait;
use ibdl_common::ahash::AHashSet;
use ibdl_common::reqwest::Client;
use ibdl_common::{
    client, extract_ext_from_url, join_tags,
    log::debug,
    post::{rating::Rating, Post, PostQueue},
    tokio::time::Instant,
    ImageBoards,
};
use std::fmt::Display;

use crate::{
    blacklist::BlacklistFilter, error::ExtractorError, websites::moebooru::models::KonachanPost,
};

use super::Extractor;

mod models;

pub struct MoebooruExtractor {
    client: Client,
    tags: Vec<String>,
    tag_string: String,
    safe_mode: bool,
    disable_blacklist: bool,
    total_removed: u64,
}

#[async_trait]
impl Extractor for MoebooruExtractor {
    fn new<S>(tags: &[S], safe_mode: bool, disable_blacklist: bool) -> Self
    where
        S: ToString + Display,
    {
        // Use common client for all connections with a set User-Agent
        let client = client!(ImageBoards::Konachan);

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
            safe_mode,
            disable_blacklist,
            total_removed: 0,
        }
    }

    async fn search(&mut self, page: usize) -> Result<PostQueue, ExtractorError> {
        let mut posts = Self::get_post_list(self, page).await?;

        if posts.is_empty() {
            return Err(ExtractorError::ZeroPosts);
        }

        posts.sort();
        posts.reverse();

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
        let blacklist = BlacklistFilter::init(
            ImageBoards::Konachan,
            &AHashSet::default(),
            self.safe_mode,
            self.disable_blacklist,
        )
        .await?;

        let mut fvec = if let Some(size) = limit {
            Vec::with_capacity(size)
        } else {
            Vec::new()
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
                println!();
                break;
            }

            let list = if !self.disable_blacklist || self.safe_mode {
                let (removed, posts) = blacklist.filter(posts);
                self.total_removed += removed;
                posts
            } else {
                posts
            };

            fvec.extend(list);

            if let Some(num) = limit {
                if fvec.len() >= num {
                    break;
                }
            }

            if size < 320 || page == 100 {
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
            posts: fvec,
            tags: self.tags.clone(),
        };

        Ok(fin)
    }

    async fn get_post_list(&self, page: usize) -> Result<Vec<Post>, ExtractorError> {
        // Get URL
        let url = format!(
            "{}?tags={}",
            ImageBoards::Konachan.post_url(),
            &self.tag_string
        );

        let items = &self
            .client
            .get(&url)
            .query(&[("page", page), ("limit", 100)])
            .send()
            .await?
            .json::<Vec<KonachanPost>>()
            .await?;

        let start = Instant::now();
        let post_list: Vec<Post> = items
            .iter()
            .filter(|c| c.file_url.is_some())
            .map(|c| {
                let url = c.file_url.clone().unwrap();

                let mut tags = AHashSet::new();

                for i in c.tags.split(' ') {
                    tags.insert(i.to_string());
                }

                Post {
                    id: c.id.unwrap(),
                    url: url.clone(),
                    md5: c.md5.clone().unwrap(),
                    extension: extract_ext_from_url!(url),
                    tags,
                    rating: Rating::from_str(&c.rating),
                }
            })
            .collect();
        let end = Instant::now();

        debug!("List size: {}", post_list.len());
        debug!("Post mapping took {:?}", end - start);

        Ok(post_list)
    }

    fn client(self) -> Client {
        self.client
    }

    fn total_removed(&self) -> u64 {
        self.total_removed
    }
}
