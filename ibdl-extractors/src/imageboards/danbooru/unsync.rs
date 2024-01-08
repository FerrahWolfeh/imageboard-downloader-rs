use ahash::{HashMap, HashMapExt};
use async_trait::async_trait;
use ibdl_common::{
    log::debug,
    post::Post,
    tokio::{
        spawn,
        sync::mpsc::{Sender, UnboundedSender},
        task::JoinHandle,
    },
    ImageBoards,
};

use crate::{
    blacklist::BlacklistFilter,
    error::ExtractorError,
    imageboards::{
        AsyncFetch, Extractor, PoolExtract, PostFetchAsync, PostFetchMethod, SinglePostFetch,
    },
};

use super::DanbooruExtractor;

// A quick alias so I can copy paste stuff faster
type ExtractorUnit = DanbooruExtractor;

#[async_trait]
impl AsyncFetch for ExtractorUnit {
    #[inline]
    fn setup_fetch_thread(
        self,
        sender_channel: UnboundedSender<Post>,
        start_page: Option<u16>,
        limit: Option<u16>,
        post_counter: Option<Sender<u64>>,
    ) -> JoinHandle<Result<u64, ExtractorError>> {
        spawn(async move {
            let mut ext = self;
            ext.async_fetch(sender_channel, start_page, limit, post_counter)
                .await
        })
    }

    async fn async_fetch(
        &mut self,
        sender_channel: UnboundedSender<Post>,
        start_page: Option<u16>,
        limit: Option<u16>,
        post_counter: Option<Sender<u64>>,
    ) -> Result<u64, ExtractorError> {
        debug!("Async extractor thread initialized");

        let blacklist = BlacklistFilter::new(
            ImageBoards::Danbooru,
            &self.excluded_tags,
            &self.download_ratings,
            self.disable_blacklist,
            !self.map_videos,
            self.selected_extension,
        )
        .await?;

        let mut pool_idxs = HashMap::with_capacity(512);

        if let Some(p_id) = self.pool_id {
            self.tag_string = format!("pool:{}", p_id);
            pool_idxs = self.fetch_pool_idxs(p_id, limit).await?;
        }

        let mut has_posts: bool = false;
        let mut total_posts_sent: u16 = 0;

        let mut page = 1;

        loop {
            let position = if let Some(n) = start_page {
                page + n
            } else {
                page
            };

            let mut posts = self.get_post_list(position).await?;
            let size = posts.len();

            if size == 0 {
                if !has_posts {
                    return Err(ExtractorError::ZeroPosts);
                }

                break;
            }

            if !self.extra_tags.is_empty() {
                posts.retain(|post| {
                    post.tags
                        .iter()
                        .all(|tag| self.extra_tags.contains(&tag.tag()))
                });
            }

            let mut list = if !(self.disable_blacklist || self.download_ratings.is_empty()) {
                let (removed, posts) = blacklist.filter(posts);
                self.total_removed += removed;
                posts
            } else {
                posts
            };

            if !has_posts && !list.is_empty() {
                has_posts = true;
            }

            for i in list.iter_mut() {
                if let Some(num) = limit {
                    if total_posts_sent >= num {
                        break;
                    }
                }

                if self.pool_id.is_some() {
                    if let Some(page_num) = pool_idxs.get(&i.id) {
                        i.id = *page_num as u64;
                    } else {
                        continue;
                    }
                }

                sender_channel.send(i.clone())?;
                total_posts_sent += 1;
                if let Some(counter) = &post_counter {
                    counter.send(1).await?;
                }
            }

            if let Some(num) = limit {
                if total_posts_sent >= num {
                    debug!("Target post count of {} reached.", num);
                    break;
                }
            }

            if page == 100 {
                debug!("Max number of pages reached");
                break;
            }

            page += 1;
        }

        debug!("Terminating thread.");
        Ok(self.total_removed)
    }
}

impl PostFetchAsync for ExtractorUnit {
    fn setup_async_post_fetch(
        self,
        post_channel: UnboundedSender<Post>,
        method: PostFetchMethod,
        length_channel: Sender<u64>,
    ) -> JoinHandle<Result<u64, ExtractorError>> {
        spawn(async move {
            let mut unit = self;
            match method {
                PostFetchMethod::Single(p_id) => {
                    post_channel.send(unit.get_post(p_id).await?)?;
                    length_channel.send(1).await?;
                }
                PostFetchMethod::Multiple(p_ids) => {
                    for p_id in p_ids {
                        post_channel.send(unit.get_post(p_id).await?)?;
                        length_channel.send(1).await?;
                    }
                }
            }
            Ok(0)
        })
    }
}
