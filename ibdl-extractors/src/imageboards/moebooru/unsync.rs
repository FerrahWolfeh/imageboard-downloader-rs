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
    imageboards::{AsyncFetch, Extractor},
};

use super::MoebooruExtractor;

// A quick alias so I can copy paste stuff faster
type ExtractorUnit = MoebooruExtractor;

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
        let blacklist = BlacklistFilter::new(
            ImageBoards::Konachan,
            &self.excluded_tags,
            &self.download_ratings,
            self.disable_blacklist,
            !self.map_videos,
            self.selected_extension,
        )
        .await?;

        let mut has_posts: bool = false;
        let mut total_posts_sent: u16 = 0;

        let mut page = 1;

        debug!("Async extractor thread initialized");

        loop {
            let position = if let Some(n) = start_page {
                page + n
            } else {
                page
            };

            let posts = self.get_post_list(position).await?;
            let size = posts.len();

            if size == 0 {
                if !has_posts {
                    return Err(ExtractorError::ZeroPosts);
                }

                break;
            }

            let list = if !self.disable_blacklist || !self.download_ratings.is_empty() {
                let (removed, posts) = blacklist.filter(posts);
                self.total_removed += removed;
                posts
            } else {
                posts
            };

            if !has_posts && !list.is_empty() {
                has_posts = true;
            }

            for i in list {
                if let Some(num) = limit {
                    if total_posts_sent >= num {
                        break;
                    }
                }

                sender_channel.send(i)?;
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
                break;
            }

            page += 1;
        }

        debug!("Terminating thread.");
        Ok(self.total_removed)
    }
}
