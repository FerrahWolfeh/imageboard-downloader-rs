//! Modules that work by parsing post info from a imageboard API into a list of [Posts](ibdl_common::post).
//! # Extractors
//!
//! All modules implementing [`Extractor`] work by connecting to a imageboard website, searching for posts with the tags supplied and parsing all of them into a [`PostQueue`](PostQueue).
//!
#![deny(clippy::nursery)]
use crate::{
    auth::{AuthState, ImageboardConfig},
    blacklist::BlacklistFilter,
    extractor_config::ServerConfig,
    prelude::{AsyncFetch, Auth, PoolExtract, PostFetchAsync, PostFetchMethod, SinglePostFetch},
};
use ahash::HashMap;
use ibdl_common::{
    client,
    log::debug,
    post::{extension::Extension, rating::Rating, Post, PostQueue},
    reqwest::{Client, Method},
    ImageBoards,
};
use serde::de::DeserializeOwned;
use std::{
    fmt::Display,
    time::{Duration, Instant},
};
use tokio::{
    spawn,
    sync::mpsc::{Sender, UnboundedSender},
    task::JoinHandle,
    time::sleep,
};

use crate::error::ExtractorError;

use caps::ExtractorFeatures;

pub mod caps;
pub mod common;

/// Trait defining site-specific API interactions and mappings.
pub trait SiteApi: Send + Sync {
    /// Type for deserializing a list of posts from the API.
    type PostListResponse: DeserializeOwned;
    /// Type for deserializing a single post from the API.
    type SinglePostResponse: DeserializeOwned;
    /// Type for deserializing the response from a pool details API endpoint.
    type PoolDetailsResponse: DeserializeOwned;

    /// Deserializes the raw string data from a post list API endpoint.
    fn deserialize_post_list(&self, data: &str) -> Result<Self::PostListResponse, ExtractorError>;

    /// Deserializes the raw string data from a single post API endpoint.
    fn deserialize_single_post(
        &self,
        data: &str,
    ) -> Result<Self::SinglePostResponse, ExtractorError>;

    /// Maps the deserialized API list response to a `Vec<Post>`.
    fn map_post_list_response(
        &self,
        response: Self::PostListResponse,
    ) -> Result<Vec<Post>, ExtractorError>;

    /// Maps the deserialized API single post response to a `Post`.
    fn map_single_post_response(
        &self,
        response: Self::SinglePostResponse,
    ) -> Result<Post, ExtractorError>;

    /// Gets the URL for a single post.
    /// `base_url` is `ServerConfig.post_url`.
    fn single_post_url(&self, base_url: &str, post_id: u32) -> String;

    /// Gets the URL for fetching a list of posts.
    /// `base_url` is `ServerConfig.post_list_url`.
    /// `page_num` is 1-indexed.
    /// `limit` is the requested number of posts.
    /// `tags_query_string` is the already processed tag string for the API.
    fn posts_url(
        &self,
        base_url: &str,
        page_num: u16,
        limit: u16,
        tags_query_string: &str,
    ) -> String;

    /// Processes input tags for API queries and display.
    /// Returns `(tags_for_api_query_string, tags_for_post_queue_vec)`.
    /// The implementor is responsible for any site-specific tag handling
    /// (e.g. Danbooru's `extra_tags` should be handled internally by the `SiteApi` impl).
    fn process_tags(&mut self, input_tags: &[String]) -> (String, Vec<String>);

    /// Max number of pages to crawl in `full_search`.
    fn full_search_page_limit(&self) -> u16;

    /// Condition to break `full_search` loop based on posts fetched this page.
    fn full_search_post_limit_break_condition(&self, posts_fetched_this_page: usize) -> bool;

    /// Optional delay between API calls in `full_search` loop.
    fn full_search_api_call_delay(&self) -> Option<Duration>;

    /// Delay between API calls in `SinglePostFetch::get_posts` loop.
    fn multi_get_post_api_call_delay(&self) -> Duration;

    /// The `ImageBoards` enum variant for this site.
    fn imageboard_type(&self) -> ImageBoards;

    /// The features supported by this extractor.
    fn features() -> ExtractorFeatures;

    // New methods for pool operations
    /// Gets the URL for fetching pool details (which includes post IDs).
    /// `base_url` is `ServerConfig.pool_url`.
    fn pool_details_url(&self, base_url: &str, pool_id: u32) -> String;

    /// Deserializes the raw string data from a pool details API endpoint.
    fn deserialize_pool_details_response(
        &self,
        data: &str,
    ) -> Result<Self::PoolDetailsResponse, ExtractorError>;

    /// Maps the deserialized pool details response to a `HashMap` of post IDs and their 0-based order index.
    fn map_pool_details_to_post_ids_with_order(
        &self,
        response: Self::PoolDetailsResponse,
    ) -> Result<HashMap<u64, usize>, ExtractorError>;

    /// Parses a raw JSON string (presumably from a pool API) into a Vec of post IDs.
    /// Order should be preserved if the JSON implies an order (e.g., a JSON array).
    fn parse_post_ids_from_pool_json_str(&self, raw_json: &str)
        -> Result<Vec<u64>, ExtractorError>;
}

/// Generic post extractor for imageboards.
#[derive(Debug, Clone)]
pub struct PostExtractor<S: SiteApi> {
    client: Client,
    tags_for_query: String,
    tags_for_post_queue: Vec<String>,
    auth_state: AuthState,
    auth_config: ImageboardConfig,
    download_ratings: Vec<Rating>,
    disable_blacklist: bool,
    total_removed: u64,
    map_videos: bool,
    user_excluded_tags: Vec<String>,
    selected_extension: Option<Extension>,
    server_cfg: ServerConfig,
    pool_config: Option<PoolDownloadConfig>,
    site_api: S,
}

/// Configuration for a pool-based download.
#[derive(Debug, Clone)]
struct PoolDownloadConfig {
    pool_id: u32,
    last_first: bool,
}

impl<S: SiteApi> PostExtractor<S> {
    /// Creates a new `PostExtractor` with the given site-specific API handler and configuration.
    /// This is the main constructor to be called by specific extractor initializers that provide
    /// their own `SiteApi` implementation and `ServerConfig`.
    pub fn new<T>(
        tags_raw: &[T],
        download_ratings: &[Rating],
        disable_blacklist: bool,
        map_videos: bool,
        mut site_api: S, // Consumes the site_api instance
        server_cfg: ServerConfig,
    ) -> Self
    where
        T: ToString + Display,
    {
        let client_cfg = server_cfg.clone();
        let client = client!(client_cfg);

        let initial_tags: Vec<String> = tags_raw
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        let (tags_for_query, tags_for_post_queue) = site_api.process_tags(&initial_tags);

        debug!("Processed Tags for API Query: {}", tags_for_query);
        debug!("Tags for PostQueue: {:?}", tags_for_post_queue);

        Self {
            client,
            tags_for_query,
            tags_for_post_queue,
            auth_state: AuthState::NotAuthenticated,
            auth_config: ImageboardConfig::default(),
            download_ratings: download_ratings.to_vec(),
            disable_blacklist,
            total_removed: 0,
            map_videos,
            user_excluded_tags: vec![],
            selected_extension: None,
            server_cfg,
            pool_config: None,
            site_api,
        }
    }

    pub async fn search(&self, page: u16) -> Result<PostQueue, ExtractorError> {
        let mut posts = self.get_post_list(page, None).await?;

        if posts.is_empty() {
            return Err(ExtractorError::ZeroPosts);
        }

        posts.sort();
        posts.reverse();

        Ok(PostQueue {
            imageboard: self.server_cfg.server,
            client: self.client.clone(),
            posts,
            tags: self.tags_for_post_queue.clone(),
        })
    }

    pub async fn full_search(
        &mut self,
        start_page: Option<u16>,
        limit: Option<u16>,
    ) -> Result<PostQueue, ExtractorError> {
        let mut effective_blacklist_tags = self.user_excluded_tags.clone();
        if self.auth_state.is_auth() && !self.auth_config.user_data.blacklisted_tags.is_empty() {
            debug!(
                "Auth blacklist tags: {:?}",
                self.auth_config.user_data.blacklisted_tags
            );
            effective_blacklist_tags
                .extend(self.auth_config.user_data.blacklisted_tags.iter().cloned());
            effective_blacklist_tags.sort_unstable();
            effective_blacklist_tags.dedup();
        }
        debug!("Effective blacklist tags: {:?}", effective_blacklist_tags);

        let blacklist = BlacklistFilter::new(
            self.server_cfg.clone(),
            &effective_blacklist_tags,
            &self.download_ratings,
            self.disable_blacklist,
            !self.map_videos,
            self.selected_extension,
        )
        .await?;

        let mut fvec = limit.map_or_else(
            || Vec::with_capacity(self.server_cfg.max_post_limit as usize),
            |size| Vec::with_capacity(size as usize),
        );

        let mut page = 1;
        let page_limit = self.site_api.full_search_page_limit();

        loop {
            let position = start_page.map_or(page, |n| page + n - 1);
            // Ensure page number is valid
            if position == 0 {
                return Err(ExtractorError::ZeroPage);
            }

            debug!("Scanning page {}", position);

            let posts_this_page = self.get_post_list(position, limit).await?;
            let posts_count = posts_this_page.len();

            if self
                .site_api
                .full_search_post_limit_break_condition(posts_count)
                && page > 1
            // Don't break on first page if empty, get_post_list would error
            {
                debug!(
                    "Site API condition met to break search loop (posts: {}).",
                    posts_count
                );
                break;
            }

            if posts_count == 0 && !fvec.is_empty() {
                // No more posts from API, but we have some already
                break;
            }

            if posts_count == 0 && fvec.is_empty() && page > 1 {
                // No posts from API and none collected after first page
                break;
            }

            let mut list = if !self.disable_blacklist
                || !self.download_ratings.is_empty()
                || !effective_blacklist_tags.is_empty()
            {
                let (removed, posts_after_filter) = blacklist.filter(posts_this_page);
                self.total_removed += removed;
                posts_after_filter
            } else {
                posts_this_page
            };

            fvec.append(&mut list);

            if let Some(num) = limit {
                if fvec.len() >= num as usize {
                    debug!("Post limit ({}) reached.", num);
                    break;
                }
            }

            if page >= page_limit {
                debug!("Page limit ({}) reached.", page_limit);
                break;
            }

            page += 1;
            if let Some(delay) = self.site_api.full_search_api_call_delay() {
                debug!("Delaying for {:?} before next page.", delay);
                sleep(delay).await;
            }
        }

        if fvec.is_empty() {
            return Err(ExtractorError::ZeroPosts);
        }

        fvec.sort();
        fvec.reverse();

        Ok(PostQueue {
            imageboard: self.server_cfg.server,
            client: self.client.clone(),
            posts: fvec,
            tags: self.tags_for_post_queue.clone(),
        })
    }

    pub fn exclude_tags(&mut self, tags: &[String]) -> &mut Self {
        self.user_excluded_tags = tags.to_vec();
        self
    }

    pub fn force_extension(&mut self, extension: Extension) -> &mut Self {
        self.selected_extension = Some(extension);
        self
    }

    pub async fn get_post_list(
        &self,
        page: u16,
        limit_option: Option<u16>,
    ) -> Result<Vec<Post>, ExtractorError> {
        let base_post_url = self
            .server_cfg
            .post_list_url
            .as_ref()
            .ok_or(ExtractorError::UnsupportedOperation)?;

        let page_post_count = limit_option.map_or(self.server_cfg.max_post_limit, |count| {
            count.min(self.server_cfg.max_post_limit)
        });

        let final_url_string =
            self.site_api
                .posts_url(base_post_url, page, page_post_count, &self.tags_for_query);

        let mut request_builder = self.client.request(Method::GET, &final_url_string);

        if self.auth_state.is_auth() {
            debug!(
                "[AUTH] Fetching posts from page {} via URL: {}",
                page, final_url_string
            );
            request_builder = request_builder
                .basic_auth(&self.auth_config.username, Some(&self.auth_config.api_key));
        } else {
            debug!(
                "Fetching posts from page {} via URL: {}",
                page, final_url_string
            );
        }

        let req = request_builder; // All query params are now part of final_url_string

        let raw_body = req.send().await?.text().await?;
        let parsed_response = self.site_api.deserialize_post_list(&raw_body)?;

        let start_point = Instant::now();
        let posts = self.site_api.map_post_list_response(parsed_response)?;
        debug!("Post mapping took {:?}", start_point.elapsed());
        debug!("Found {} posts on page {}", posts.len(), page);
        Ok(posts)
    }

    // This method is not used by PostExtractor directly, SiteApi handles mapping.
    // It's part of the trait, so we provide a stub. Concrete SiteApi impls do the work.
    fn map_posts(&self, raw_data: &str) -> Result<Vec<Post>, ExtractorError> {
        let deserialized_list = self.site_api.deserialize_post_list(raw_data)?;
        self.site_api.map_post_list_response(deserialized_list)
    }

    pub fn client(&self) -> Client {
        self.client.clone()
    }

    const fn total_removed(&self) -> u64 {
        self.total_removed
    }

    pub fn imageboard(&self) -> ImageBoards {
        self.site_api.imageboard_type()
    }

    #[must_use]
    pub fn features() -> ExtractorFeatures {
        // This should ideally be S::features(), but Extractor::features is not tied to `self`.
        // Concrete types will override this.
        S::features()
    }

    pub fn config(&self) -> ServerConfig {
        self.server_cfg.clone()
    }
}

impl<S: SiteApi> Auth for PostExtractor<S> {
    async fn auth(&mut self, config: ImageboardConfig) -> Result<(), ExtractorError> {
        debug!(
            "Authenticating user: {}. Blacklisted tags from auth: {:?}",
            config.username, config.user_data.blacklisted_tags
        );
        self.auth_config = config;
        self.auth_state = AuthState::Authenticated;
        Ok(())
    }
}

impl<S: SiteApi + 'static> SinglePostFetch for PostExtractor<S> {
    // This method is not used by PostExtractor directly, SiteApi handles mapping.
    fn map_post(&self, raw_data: String) -> Result<Post, ExtractorError> {
        let deserialized_post = self.site_api.deserialize_single_post(&raw_data)?;
        self.site_api.map_single_post_response(deserialized_post)
    }

    async fn get_post(&mut self, post_id: u32) -> Result<Post, ExtractorError> {
        let base_url = self
            .server_cfg
            .post_url
            .as_ref()
            .ok_or(ExtractorError::UnsupportedOperation)?;
        let url = self.site_api.single_post_url(base_url, post_id);

        let mut request_builder = self.client.get(url);
        if self.auth_state.is_auth() {
            debug!("[AUTH] Fetching post {}", post_id);
            request_builder = request_builder
                .basic_auth(&self.auth_config.username, Some(&self.auth_config.api_key));
        } else {
            debug!("Fetching post {}", post_id);
        }

        let raw_body = request_builder.send().await?.text().await?;
        let parsed_response = self.site_api.deserialize_single_post(&raw_body)?;

        let start_point = Instant::now();
        let post = self.site_api.map_single_post_response(parsed_response)?;
        debug!("Single post mapping took {:?}", start_point.elapsed());
        Ok(post)
    }

    async fn get_posts(&mut self, posts_ids: &[u32]) -> Result<Vec<Post>, ExtractorError> {
        let mut pvec = Vec::with_capacity(posts_ids.len());
        let delay = self.site_api.multi_get_post_api_call_delay();

        for (idx, post_id) in posts_ids.iter().enumerate() {
            let post = self.get_post(*post_id).await?;
            pvec.push(post);

            if idx < posts_ids.len() - 1 {
                debug!("Debouncing API calls by {:?}", delay);
                sleep(delay).await;
            }
        }
        Ok(pvec)
    }
}

impl<S: SiteApi + 'static> PostExtractor<S> {
    /// Handles the logic for fetching posts from a pool.
    async fn async_fetch_pool_logic(
        &mut self,
        sender_channel: UnboundedSender<Post>,
        limit: Option<u16>,
        post_counter: Option<Sender<u64>>,
        pool_cfg: &PoolDownloadConfig,
    ) -> Result<u64, ExtractorError> {
        debug!(
            "Starting POOL download for ID: {}. last_first: {}. Limit: {:?}. Tags and start_page will be ignored.",
            pool_cfg.pool_id, pool_cfg.last_first, limit
        );

        // Fetch all post IDs and their order for the pool. Limit is applied later.
        let all_pool_posts_map = self.fetch_pool_idxs(pool_cfg.pool_id, None).await?;

        if all_pool_posts_map.is_empty() {
            debug!(
                "Pool {} is empty or could not be fetched.",
                pool_cfg.pool_id
            );
            return Ok(self.total_removed); // No posts to process
        }

        // Convert map to a Vec of (post_id, order_index) and sort by order_index
        let mut sorted_pool_items: Vec<(u64, usize)> = all_pool_posts_map.into_iter().collect();
        sorted_pool_items.sort_by_key(|&(_, order_idx)| order_idx);

        // Apply last_first if needed
        if pool_cfg.last_first {
            debug!("Reversing pool order (last_first is true)");
            sorted_pool_items.reverse();
        }

        // Apply the overall download limit (from async_fetch args)
        let limited_pool_items: Vec<(u64, usize)> = sorted_pool_items
            .into_iter()
            .take(limit.map_or(usize::MAX, |l| l as usize))
            .collect();

        if limited_pool_items.is_empty() {
            debug!("No posts to fetch from pool after applying limit or due to empty pool.");
            return Ok(self.total_removed);
        }

        debug!(
            "Fetching {} posts from pool {}",
            limited_pool_items.len(),
            pool_cfg.pool_id
        );

        let blacklist = BlacklistFilter::new(
            self.server_cfg.clone(),
            &self.user_excluded_tags,
            &self.download_ratings,
            self.disable_blacklist,
            !self.map_videos,
            self.selected_extension,
        )
        .await?;

        let mut posts_sent_this_run = 0u64;
        for (post_id_u64, _) in limited_pool_items {
            match self.get_post(u32::try_from(post_id_u64)?).await {
                Ok(post_to_filter) => {
                    let (removed_count, mut filtered_posts_vec) =
                        blacklist.filter(vec![post_to_filter]);
                    self.total_removed += removed_count;

                    if let Some(final_post) = filtered_posts_vec.pop() {
                        sender_channel.send(final_post)?;
                        posts_sent_this_run += 1;
                        if let Some(counter) = &post_counter {
                            counter.send(1).await?;
                        }
                    } else {
                        debug!("Post ID {} from pool was filtered out.", post_id_u64);
                    }
                }
                Err(e) => {
                    ibdl_common::log::error!(
                        "Failed to fetch post ID {} from pool: {:?}",
                        post_id_u64,
                        e
                    );
                    // Optionally, decide if this error should halt the process or be skipped.
                    // For now, we log and continue.
                }
            }
        }
        debug!(
            "Finished pool download. Sent {} posts. Total posts removed by blacklist: {}",
            posts_sent_this_run, self.total_removed
        );
        Ok(self.total_removed)
    }

    /// Handles the logic for fetching posts based on tags.
    async fn async_fetch_tag_logic(
        &mut self,
        sender_channel: UnboundedSender<Post>,
        start_page: Option<u16>,
        limit: Option<u16>,
        post_counter: Option<Sender<u64>>,
    ) -> Result<u64, ExtractorError> {
        // Existing tag-based search logic
        debug!(
            "Starting TAG-BASED download. Tags: '{}'. Limit: {:?}. Start Page: {:?}.",
            self.tags_for_query, limit, start_page
        );
        let blacklist = BlacklistFilter::new(
            self.server_cfg.clone(),
            &self.user_excluded_tags, // Using PostExtractor's own excluded tags
            &self.download_ratings,
            self.disable_blacklist,
            !self.map_videos,
            self.selected_extension,
        )
        .await?;

        let mut has_posts: bool = false;
        let mut total_posts_sent: u16 = 0; // This is u16, consistent with limit
        let mut page = 1;
        let page_limit = self.site_api.full_search_page_limit();

        loop {
            let position = start_page.map_or(page, |n| page + n - 1);
            if position == 0 {
                return Err(ExtractorError::ZeroPage);
            }

            let mut posts = self.get_post_list(position, limit).await?;
            let size = posts.len();

            if size == 0 {
                if !has_posts && page == 1 {
                    // Only error if first page is empty
                    return Err(ExtractorError::ZeroPosts);
                }
                debug!("No more posts found on page {}.", position);
                break;
            }

            // Positive filtering using tags_for_post_queue
            if !self.tags_for_post_queue.is_empty() {
                let required_tags = &self.tags_for_post_queue;
                posts.retain(|post| {
                    required_tags.iter().all(|required_tag_str| {
                        post.tags
                            .iter()
                            .any(|post_tag| post_tag.tag() == *required_tag_str)
                    })
                });
            }

            let list = if self.disable_blacklist
                && self.download_ratings.is_empty()
                && self.user_excluded_tags.is_empty()
            {
                // Simplified condition
                posts
            } else {
                let (removed, posts_after_filter) = blacklist.filter(posts);
                self.total_removed += removed;
                posts_after_filter
            };

            if !list.is_empty() {
                has_posts = true;
            }

            for post_item in list {
                if let Some(num) = limit {
                    if total_posts_sent >= num {
                        break;
                    }
                }

                sender_channel.send(post_item)?;
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

            if page >= page_limit {
                debug!("Max number of pages ({}) reached", page_limit);
                break;
            }

            page += 1;
            if let Some(delay) = self.site_api.full_search_api_call_delay() {
                debug!("Delaying for {:?} before next page.", delay);
                sleep(delay).await;
            }
        }

        if !has_posts {
            // If loop finished and no posts were ever processed (e.g. all filtered out)
            return Err(ExtractorError::ZeroPosts);
        }

        debug!("Terminating generic PostExtractor TAG-BASED async_fetch thread. Total posts sent: {}. Total posts removed by blacklist: {}", total_posts_sent, self.total_removed);
        Ok(self.total_removed)
    }
}

impl<S: SiteApi + 'static> AsyncFetch for PostExtractor<S> {
    async fn async_fetch(
        &mut self,
        sender_channel: UnboundedSender<Post>,
        start_page: Option<u16>,
        limit: Option<u16>,
        post_counter: std::option::Option<tokio::sync::mpsc::Sender<u64>>,
    ) -> Result<u64, ExtractorError> {
        debug!("Async extractor thread initialized for generic PostExtractor");
        if let Some(pool_cfg_clone) = self.pool_config.clone() {
            self.async_fetch_pool_logic(sender_channel, limit, post_counter, &pool_cfg_clone)
                .await
        } else {
            self.async_fetch_tag_logic(sender_channel, start_page, limit, post_counter)
                .await
        }
    }

    #[inline]
    fn setup_fetch_thread(
        mut self, // PostExtractor needs to be mutable for async_fetch
        sender_channel: UnboundedSender<Post>,
        start_page: Option<u16>,
        limit: Option<u16>,
        post_counter: Option<Sender<u64>>,
    ) -> JoinHandle<Result<u64, ExtractorError>> {
        spawn(async move {
            self.async_fetch(sender_channel, start_page, limit, post_counter)
                .await
        })
    }
}

impl<S: SiteApi + 'static> PostFetchAsync for PostExtractor<S> {
    fn setup_async_post_fetch(
        mut self, // PostExtractor needs to be mutable for get_post
        post_channel: UnboundedSender<Post>,
        method: PostFetchMethod,
        length_channel: Sender<u64>,
    ) -> JoinHandle<Result<u64, ExtractorError>> {
        spawn(async move {
            match method {
                PostFetchMethod::Single(p_id) => {
                    post_channel.send(self.get_post(p_id).await?)?;
                    length_channel.send(1).await?;
                }
                PostFetchMethod::Multiple(p_ids) => {
                    let delay = self.site_api.multi_get_post_api_call_delay();
                    for (idx, p_id) in p_ids.iter().enumerate() {
                        post_channel.send(self.get_post(*p_id).await?)?;
                        length_channel.send(1).await?;
                        if idx < p_ids.len() - 1 {
                            debug!("Debouncing API calls by {:?}", delay);
                            sleep(delay).await;
                        }
                    }
                }
            }
            Ok(0) // total_removed is not applicable here in the same way as main fetch
        })
    }
}

impl<S: SiteApi + 'static> PoolExtract for PostExtractor<S> {
    async fn fetch_pool_idxs(
        &mut self,
        pool_id: u32,
        limit_opt: Option<u16>,
    ) -> Result<HashMap<u64, usize>, ExtractorError> {
        let base_url = self
            .server_cfg
            .pool_idx_url
            .as_ref()
            .ok_or(ExtractorError::UnsupportedOperation)?;

        let url = self.site_api.pool_details_url(base_url, pool_id);
        debug!("Fetching pool details from URL: {}", url);

        let mut request_builder = self.client.get(&url);

        if self.auth_state.is_auth() {
            debug!(
                "[AUTH] Fetching pool {} with user {}",
                pool_id, self.auth_config.username
            );
            request_builder = request_builder
                .basic_auth(&self.auth_config.username, Some(&self.auth_config.api_key));
        }

        let raw_body = request_builder.send().await?.text().await?;
        let deserialized_response = self.site_api.deserialize_pool_details_response(&raw_body)?;

        let mut post_map = self
            .site_api
            .map_pool_details_to_post_ids_with_order(deserialized_response)?;

        if let Some(limit_val) = limit_opt {
            if post_map.len() > limit_val as usize {
                // Sort by pool order (value in HashMap) and take the top 'limit_val' items
                let mut sorted_posts: Vec<(u64, usize)> = post_map.into_iter().collect();
                sorted_posts.sort_by_key(|&(_, order_idx)| order_idx);

                post_map = sorted_posts.into_iter().take(limit_val as usize).collect();
            }
        }

        Ok(post_map)
    }

    fn parse_pool_ids(&self, raw_json: String) -> Result<Vec<u64>, ExtractorError> {
        self.site_api.parse_post_ids_from_pool_json_str(&raw_json)
    }

    fn setup_pool_download(&mut self, pool_id: Option<u32>, last_first: bool) {
        if let Some(id) = pool_id {
            self.pool_config = Some(PoolDownloadConfig {
                pool_id: id,
                last_first,
            });
            debug!(
                "PostExtractor configured for pool download: ID {}, last_first: {}",
                id, last_first
            );
        } else {
            self.pool_config = None;
            debug!("PostExtractor pool download configuration cleared.");
        }
    }
}
