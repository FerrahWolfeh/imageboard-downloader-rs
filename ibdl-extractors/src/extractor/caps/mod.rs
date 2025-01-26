use crate::auth::ImageboardConfig;
use crate::error::ExtractorError;
use ahash::HashMap;
use bitflags::bitflags;
use ibdl_common::post::Post;
use ibdl_common::tokio::sync::mpsc::{Sender, UnboundedSender};
use ibdl_common::tokio::task::JoinHandle;
use std::future::Future;

pub type ExtractorThreadHandle = JoinHandle<Result<u64, ExtractorError>>;

bitflags! {
    pub struct ExtractorFeatures: u8 {
        const AsyncFetch = 0b0000_0001;
        const TagSearch = 0b0000_0010;
        const SinglePostFetch = 0b0000_0100;
        const PoolDownload = 0b0000_1000;
        const Auth = 0b0001_0000;
    }
}

/// Authentication capability for imageboard websites. Implies the Extractor is able to use a user-defined blacklist
pub trait Auth {
    /// Authenticates to the imageboard using the supplied [`Config`](ImageboardConfig)
    fn auth(
        &mut self,
        config: ImageboardConfig,
    ) -> impl Future<Output = Result<(), ExtractorError>> + Send;
}

/// Capability for the extractor asynchronously send posts through a [`unbounded_channel`](ibdl_common::tokio::sync::mpsc::unbounded_channel) to another thread.
pub trait AsyncFetch {
    /// Similar to [`full_search`](Extractor::full_search) in functionality, but instead of returning a [`PostQueue`](PostQueue), sends posts asynchronously through a channel.
    fn async_fetch(
        &mut self,
        sender_channel: UnboundedSender<Post>,
        start_page: Option<u16>,
        limit: Option<u16>,
        post_counter: Option<Sender<u64>>,
    ) -> impl Future<Output = Result<u64, ExtractorError>> + Send;

    /// High-level convenience thread builder for [`async_fetch`](crate::websites::AsyncFetch::async_fetch)
    fn setup_fetch_thread(
        self,
        sender_channel: UnboundedSender<Post>,
        start_page: Option<u16>,
        limit: Option<u16>,
        post_counter: Option<Sender<u64>>,
    ) -> JoinHandle<Result<u64, ExtractorError>>;
}

#[derive(Debug, Clone)]
pub enum PostFetchMethod {
    Single(u32),
    Multiple(Vec<u32>),
}

pub trait SinglePostFetch {
    /// This is a separate lower level function to map a single post by feeding the imageboard's post representation.
    fn map_post(&self, raw_json: String) -> Result<Post, ExtractorError>;

    /// Fetch one single post from the imageboard.
    fn get_post(
        &mut self,
        post_id: u32,
    ) -> impl Future<Output = Result<Post, ExtractorError>> + Send;

    /// Fetch n posts from the imageboard.
    fn get_posts(
        &mut self,
        posts: &[u32],
    ) -> impl Future<Output = Result<Vec<Post>, ExtractorError>> + Send;
}

pub trait PostFetchAsync {
    fn setup_async_post_fetch(
        self,
        post_channel: UnboundedSender<Post>,
        method: PostFetchMethod,
        length_channel: Sender<u64>,
    ) -> JoinHandle<Result<u64, ExtractorError>>;
}

pub trait PoolExtract {
    fn fetch_pool_idxs(
        &mut self,
        pool_id: u32,
        limit: Option<u16>,
    ) -> impl Future<Output = Result<HashMap<u64, usize>, ExtractorError>> + Send;

    fn parse_pool_ids(&self, raw_json: String) -> Result<Vec<u64>, ExtractorError>;

    fn setup_pool_download(&mut self, pool_id: Option<u32>, last_first: bool);
}
