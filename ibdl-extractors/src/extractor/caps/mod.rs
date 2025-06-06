//! Defines traits that represent various capabilities or features an imageboard extractor can implement.
//!
//! These capabilities allow for a more fine-grained understanding and utilization of an extractor's functionalities.
use crate::auth::ImageboardConfig;
use crate::error::ExtractorError;
use ahash::HashMap;
use bitflags::bitflags;
use ibdl_common::post::Post;
use std::future::Future;
use tokio::sync::mpsc::{Sender, UnboundedSender};

#[cfg(not(target_arch = "wasm32"))]
use tokio::task::JoinHandle;

/// A type alias for a `JoinHandle` returned by asynchronous extractor operations.
///
/// The `JoinHandle` resolves to a `Result` containing a `u64` (often representing a count,
/// like posts processed or removed) or an `ExtractorError`.
#[cfg(not(target_arch = "wasm32"))]
pub type ExtractorThreadHandle = JoinHandle<Result<u64, ExtractorError>>;

bitflags! {
        pub struct ExtractorFeatures: u8 {
        /// Indicates that the extractor can fetch posts asynchronously, sending them through a channel.
        const AsyncFetch = 0b0000_0001;
        /// Indicates that the extractor supports searching for posts based on tags.
        const TagSearch = 0b0000_0010;
        /// Indicates that the extractor can fetch a single post by its ID.
        const SinglePostFetch = 0b0000_0100;
        /// Indicates that the extractor supports downloading posts from a "pool" (a curated collection of posts).
        const PoolDownload = 0b0000_1000;
        /// Indicates that the extractor supports authentication with the imageboard.
        const Auth = 0b0001_0000;
    }

}

/// Defines the capability for an extractor to authenticate with an imageboard.
///
/// Implementing this trait allows the extractor to use user credentials (like username and API key)
/// for requests, potentially accessing restricted content or user-specific features like personalized blacklists.
pub trait Auth {
    /// Authenticates the extractor with the imageboard using the provided configuration.
    ///
    /// After successful authentication, subsequent requests made by the extractor
    /// should use the authenticated session.
    ///
    /// # Arguments
    /// * `config`: An `ImageboardConfig` struct containing the necessary
    ///   credentials (e.g., username, API key) and potentially user-specific settings.
    ///
    /// # Returns
    /// A `Future` that resolves to `Result<(), ExtractorError>`. `Ok(())` indicates successful
    /// authentication, while an `Err` indicates an authentication failure.
    #[cfg(not(target_arch = "wasm32"))]
    fn auth(
        &mut self,
        config: ImageboardConfig,
    ) -> impl Future<Output = Result<(), ExtractorError>> + Send;

    #[cfg(target_arch = "wasm32")]
    fn auth(
        &mut self,
        config: ImageboardConfig,
    ) -> impl Future<Output = Result<(), ExtractorError>>;
}

/// Defines the capability for an extractor to fetch posts asynchronously and send them
/// through a channel to another part of the application (e.g., a downloader).
pub trait AsyncFetch {
    /// Asynchronously fetches posts based on search criteria (tags, page, limit) and
    /// sends them one by one through the provided `sender_channel`.
    ///
    /// This method is suitable for processing large numbers of posts without collecting them
    /// all in memory at once.
    ///
    /// # Arguments
    /// * `sender_channel`: An `UnboundedSender<Post>`
    ///   to which fetched `Post` objects will be sent.
    /// * `start_page`: An optional page number (1-indexed) to begin fetching from.
    /// * `limit`: An optional maximum number of posts to fetch.
    /// * `post_counter`: An optional `Sender<u64>`
    ///   to report the count of posts successfully sent through `sender_channel`.
    ///
    /// # Returns
    /// A `Future` that resolves to `Result<u64, ExtractorError>`. The `u64` on success typically
    /// represents the total number of posts removed by blacklisting during the fetch operation.
    #[cfg(not(target_arch = "wasm32"))]
    fn async_fetch(
        &mut self,
        sender_channel: UnboundedSender<Post>,
        start_page: Option<u16>,
        limit: Option<u16>,
        post_counter: Option<Sender<u64>>,
    ) -> impl Future<Output = Result<u64, ExtractorError>> + Send;

    #[cfg(target_arch = "wasm32")]
    fn async_fetch(
        &mut self,
        sender_channel: UnboundedSender<Post>,
        start_page: Option<u16>,
        limit: Option<u16>,
        post_counter: Option<Sender<u64>>,
    ) -> impl Future<Output = Result<u64, ExtractorError>>;

    /// A high-level convenience method to spawn the `async_fetch` operation in a new Tokio task.
    ///
    /// This consumes the extractor instance (`self`) and returns a `JoinHandle` for the spawned task.
    ///
    /// # Arguments
    /// * `self`: The extractor instance.
    /// * `sender_channel`: Channel to send fetched posts to.
    /// * `start_page`: Optional starting page for the fetch.
    /// * `limit`: Optional limit on the number of posts to fetch.
    /// * `post_counter`: Optional channel to report the count of sent posts.
    ///
    /// # Returns
    /// An [`ExtractorThreadHandle`], which is a `JoinHandle` for the asynchronous fetch task.
    #[cfg(not(target_arch = "wasm32"))]
    fn setup_fetch_thread(
        self,
        sender_channel: UnboundedSender<Post>,
        start_page: Option<u16>,
        limit: Option<u16>,
        post_counter: Option<Sender<u64>>,
    ) -> JoinHandle<Result<u64, ExtractorError>>;
}

/// Specifies the method for fetching one or more posts by their IDs.
#[derive(Debug, Clone)]
pub enum PostFetchMethod {
    /// Fetch a single post by its ID.
    Single(u32),
    /// Fetch multiple posts by their IDs.
    Multiple(Vec<u32>),
}

/// Defines the capability for an extractor to fetch individual posts by their ID
/// or to map raw post data to the common `Post` struct.
pub trait SinglePostFetch {
    /// Maps raw string data, typically a JSON response from an imageboard's API for a single post,
    /// into the common `Post` struct.
    ///
    /// # Arguments
    /// * `raw_json`: A `String` containing the raw, site-specific representation of a post.
    ///
    /// # Returns
    /// A `Result` containing the mapped `Post` on success, or an `ExtractorError` if
    /// deserialization or mapping fails.
    fn map_post(&self, raw_json: String) -> Result<Post, ExtractorError>;

    /// Fetches a single post from the imageboard by its unique ID.
    ///
    /// # Arguments
    /// * `post_id`: The ID of the post to fetch.
    ///
    /// # Returns
    /// A `Future` that resolves to `Result<Post, ExtractorError>`, containing the fetched `Post`
    /// on success or an error if the post cannot be found or an API error occurs.
    #[cfg(not(target_arch = "wasm32"))]
    fn get_post(
        &mut self,
        post_id: u32,
    ) -> impl Future<Output = Result<Post, ExtractorError>> + Send;

    #[cfg(target_arch = "wasm32")]
    fn get_post(&mut self, post_id: u32) -> impl Future<Output = Result<Post, ExtractorError>>;

    /// Fetches multiple posts from the imageboard by their unique IDs.
    ///
    /// # Arguments
    /// * `posts`: A slice of `u32` representing the IDs of the posts to fetch.
    ///
    /// # Returns
    /// A `Future` that resolves to `Result<Vec<Post>, ExtractorError>`, containing a vector
    /// of the fetched `Post` objects on success or an error if any post cannot be fetched
    /// or an API error occurs.
    #[cfg(not(target_arch = "wasm32"))]
    fn get_posts(
        &mut self,
        posts: &[u32],
    ) -> impl Future<Output = Result<Vec<Post>, ExtractorError>> + Send;

    #[cfg(target_arch = "wasm32")]
    fn get_posts(
        &mut self,
        posts: &[u32],
    ) -> impl Future<Output = Result<Vec<Post>, ExtractorError>>;
}

/// Defines the capability for an extractor to set up an asynchronous task
/// for fetching one or more specific posts by their IDs.
pub trait PostFetchAsync {
    /// Spawns a new Tokio task to asynchronously fetch one or more posts as specified by `method`.
    ///
    /// Fetched posts are sent through `post_channel`, and the count of successfully
    /// fetched posts is sent through `length_channel`.
    ///
    /// # Arguments
    /// * `self`: The extractor instance, consumed by this method.
    /// * `post_channel`: An `UnboundedSender<Post>`
    ///   to send the fetched `Post`(s) to.
    /// * `method`: A [`PostFetchMethod`] indicating whether to fetch a single post or multiple posts.
    /// * `length_channel`: A `Sender<u64>`
    ///   to report the number of posts successfully fetched and sent.
    ///
    /// # Returns
    /// An [`ExtractorThreadHandle`]. The `u64` in the `Result` typically signifies a status or count;
    /// for instance, `Ok(0)` might indicate successful completion of sending all requested posts,
    /// as `total_removed` (from blacklist filtering) isn't the primary concern here.
    #[cfg(not(target_arch = "wasm32"))]
    fn setup_async_post_fetch(
        self,
        post_channel: UnboundedSender<Post>,
        method: PostFetchMethod,
        length_channel: Sender<u64>,
    ) -> JoinHandle<Result<u64, ExtractorError>>;
}

/// Defines the capability for an extractor to interact with "pools" on an imageboard.
/// Pools are typically curated collections or albums of posts.
pub trait PoolExtract {
    /// Fetches the IDs of posts belonging to a specific pool, along with their order within the pool.
    ///
    /// # Arguments
    /// * `pool_id`: The ID of the pool to fetch.
    /// * `limit`: An optional limit on the number of post IDs to retrieve from the pool.
    ///
    /// # Returns
    /// A `Future` that resolves to `Result<HashMap<u64, usize>, ExtractorError>`.
    /// The `HashMap` maps post IDs (`u64`) to their 0-indexed order (`usize`) within the pool.
    #[cfg(not(target_arch = "wasm32"))]
    fn fetch_pool_idxs(
        &mut self,
        pool_id: u32,
        limit: Option<u16>,
    ) -> impl Future<Output = Result<HashMap<u64, usize>, ExtractorError>> + Send;

    #[cfg(target_arch = "wasm32")]
    fn fetch_pool_idxs(
        &mut self,
        pool_id: u32,
        limit: Option<u16>,
    ) -> impl Future<Output = Result<HashMap<u64, usize>, ExtractorError>>;

    /// Parses a raw JSON string, presumably from a pool API endpoint, into a `Vec<u64>` of post IDs.
    /// The order of IDs in the returned vector should reflect their order in the pool if specified by the API.
    ///
    /// # Arguments
    /// * `raw_json`: A `String` containing the raw JSON response from a pool API.
    ///
    /// # Returns
    /// A `Result` containing a `Vec<u64>` of post IDs on success, or an `ExtractorError` if parsing fails.
    fn parse_pool_ids(&self, raw_json: String) -> Result<Vec<u64>, ExtractorError>;

    /// Configures the extractor for downloading posts from a specific pool or clears existing pool configuration.
    ///
    /// # Arguments
    /// * `pool_id`: An `Option<u32>`. If `Some(id)`, the extractor is configured to download posts from the pool with that `id`.
    ///   If `None`, any existing pool download configuration is cleared, and the extractor will revert to tag-based searching.
    /// * `last_first`: If `true` and `pool_id` is `Some`, posts from the pool should be processed or downloaded
    ///   in reverse order (i.e., the last post in the pool's sequence is processed first).
    fn setup_pool_download(&mut self, pool_id: Option<u32>, last_first: bool);
}
