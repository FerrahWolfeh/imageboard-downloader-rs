//! Post extractor for `https://konachan.com` and other Moebooru imageboards
use crate::imageboards::extractors::error::ExtractorError;
use crate::imageboards::extractors::moebooru::models::KonachanPost;
use crate::imageboards::post::{rating::Rating, Post, PostQueue};
use crate::imageboards::ImageBoards;
use crate::{client, extract_ext_from_url, join_tags};
use ahash::AHashSet;
use async_trait::async_trait;
use log::debug;
use reqwest::Client;
use std::fmt::Display;
use tokio::time::Instant;

use super::blacklist::blacklist_filter;
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
                    ImageBoards::Konachan,
                    &mut posts,
                    &AHashSet::default(),
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
        }

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

impl MoebooruExtractor {
    async fn validate_tags(&self) -> Result<(), ExtractorError> {
        let count_endpoint = format!(
            "{}?tags={}",
            ImageBoards::Konachan.post_url(),
            &self.tag_string
        );

        // Get an estimate of total posts and pages to search
        let count = &self
            .client
            .get(&count_endpoint)
            .send()
            .await?
            .json::<Vec<KonachanPost>>()
            .await?;

        // Bail out if no posts are found
        if count.is_empty() {
            return Err(ExtractorError::ZeroPosts);
        }
        debug!("Tag list is valid");

        Ok(())
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
}
