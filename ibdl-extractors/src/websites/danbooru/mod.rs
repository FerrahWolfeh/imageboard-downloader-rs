//! Post extractor for `https://danbooru.donmai.us`
//!
//! The danbooru extractor has the following features:
//! - Authentication
//! - Native blacklist (defined in user profile page)
//!
use self::models::DanbooruPost;

use super::{AsyncFetch, Auth, Extractor};
use crate::{blacklist::BlacklistFilter, error::ExtractorError};
use async_trait::async_trait;
use ibdl_common::{
    auth::ImageboardConfig,
    client, join_tags,
    log::debug,
    post::{rating::Rating, Post, PostQueue},
    reqwest::Client,
    tokio::{spawn, sync::mpsc::UnboundedSender, task::JoinHandle, time::Instant},
    ImageBoards,
};
use ibdl_common::{serde_json, tokio};
use std::fmt::Display;
use std::sync::Mutex;

mod models;

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
}

#[async_trait]
impl AsyncFetch for DanbooruExtractor {
    #[inline]
    fn setup_fetch_thread(
        self,
        sender_channel: UnboundedSender<Post>,
        start_page: Option<u16>,
        limit: Option<u16>,
    ) -> JoinHandle<Result<(), ExtractorError>> {
        spawn(async move {
            let mut ext = self;
            ext.async_fetch(sender_channel, start_page, limit).await
        })
    }

    async fn async_fetch(
        &mut self,
        sender_channel: UnboundedSender<Post>,
        start_page: Option<u16>,
        limit: Option<u16>,
    ) -> Result<(), ExtractorError> {
        let blacklist = BlacklistFilter::init(
            ImageBoards::Danbooru,
            &self.auth.user_data.blacklisted_tags,
            &self.download_ratings,
            self.disable_blacklist,
        )
        .await?;

        let mut total_posts_fetched: u16 = 0;
        let mut total_posts_sent: u16 = 0;

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

            let list = if !self.disable_blacklist || !self.download_ratings.is_empty() {
                let (removed, posts) = blacklist.filter(posts);
                self.total_removed += removed;
                posts
            } else {
                posts
            };

            total_posts_fetched += list.len() as u16;

            for i in list {
                if let Some(num) = limit {
                    if total_posts_sent >= num {
                        break;
                    }
                }

                sender_channel.send(i)?;
                total_posts_sent += 1;
            }

            if page == 100 {
                break;
            }

            page += 1;
        }

        if total_posts_fetched == 0 {
            return Err(ExtractorError::ZeroPosts);
        }

        Ok(())
    }
}

#[async_trait]
impl Extractor for DanbooruExtractor {
    fn new<S>(tags: &[S], download_ratings: &[Rating], disable_blacklist: bool) -> Self
    where
        S: ToString + Display,
    {
        // Use common client for all connections with a set User-Agent
        let client = client!(ImageBoards::Danbooru);

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
            imageboard: ImageBoards::Danbooru,
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
            ImageBoards::Danbooru,
            &self.auth.user_data.blacklisted_tags,
            &self.download_ratings,
            self.disable_blacklist,
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

        let end_iter = Instant::now();

        debug!("List size: {}", mtx.len());
        debug!("Post mapping took {:?}", end_iter - start_point);
        Ok(mtx)
    }

    fn map_posts(&self, raw_json: String) -> Result<Vec<Post>, ExtractorError> {
        let parsed_json: Vec<DanbooruPost> =
            serde_json::from_str::<Vec<DanbooruPost>>(raw_json.as_str()).unwrap();

        let batch = parsed_json.into_iter().filter(|c| c.file_url.is_some());

        let posts: Mutex<Vec<Post>> = Mutex::new(Vec::with_capacity(batch.size_hint().0));

        batch.for_each(|c| {
            let tags = c.tag_string.unwrap();
            let tag_list_iter = tags.split(' ');
            let mut tag_list = Vec::with_capacity(tag_list_iter.size_hint().0);

            tag_list_iter.for_each(|i| {
                tag_list.push(i.to_string());
            });

            let rt = c.rating.unwrap();
            let rating = if rt == "s" {
                Rating::Questionable
            } else {
                Rating::from_rating_str(&rt)
            };

            let unit = Post {
                id: c.id.unwrap(),
                md5: c.md5.unwrap(),
                url: c.file_url.unwrap(),
                extension: c.file_ext.unwrap(),
                tags: tag_list,
                rating,
            };

            posts.lock().unwrap().push(unit);
        });

        let mtx = posts.lock().unwrap().clone();
        drop(posts);
        Ok(mtx)
    }

    fn client(self) -> Client {
        self.client
    }

    fn total_removed(&self) -> u64 {
        self.total_removed
    }
}

#[async_trait]
impl Auth for DanbooruExtractor {
    async fn auth(&mut self) -> Result<(), ExtractorError> {
        if let Some(creds) = ImageBoards::Danbooru.read_config_from_fs().await? {
            self.auth = creds;
            self.auth_state = true;
            return Ok(());
        }

        self.auth_state = false;
        Ok(())
    }
}

#[tokio::test]
async fn test_async_sender() {
    use futures::stream::StreamExt;
    use ibdl_common::tokio::join;
    use ibdl_common::tokio::sync::mpsc::unbounded_channel;
    use ibdl_common::tokio::task::spawn;
    use tokio_stream::wrappers::UnboundedReceiverStream;

    let danbooru_search = DanbooruExtractor::new(
        &["ookami_mio"],
        &[
            Rating::Safe,
            Rating::Unknown,
            Rating::Explicit,
            Rating::Questionable,
        ],
        false,
    );

    let (sender, receiver) = unbounded_channel();

    let fetch_thread = spawn(async move {
        let mut dbs = danbooru_search;

        dbs.async_fetch(sender, None, Some(1500)).await.unwrap();
    });

    let download_thread = spawn(async move {
        let unord_stream = UnboundedReceiverStream::new(receiver);
        unord_stream
            .map(|u| {
                println!("{:#?}", u);
                tokio::time::sleep(std::time::Duration::from_millis(500))
            })
            .buffer_unordered(10)
            .for_each(|_| async {})
            .await;
    });

    let (one, two) = join!(fetch_thread, download_thread);

    assert!(one.is_ok());
    assert!(two.is_ok());
}
