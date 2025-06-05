//! Modules that work by parsing post info from a imageboard API into a list of [Posts](ibdl_common::post).
//! Core module for defining and implementing post extraction logic from imageboard APIs.
//!
//! # Extractors
//!
//! This module provides the generic [`PostExtractor`](crate::extractor::PostExtractor) struct and the [`SiteApi`](crate::extractor::SiteApi) trait.
//! `PostExtractor` handles the common logic for fetching, filtering, and queuing posts,
//! while `SiteApi` defines the site-specific operations required to interact with a particular imageboard's API.
use crate::{
    auth::{AuthState, ImageboardConfig},
    blacklist::BlacklistFilter,
    extractor_config::ServerConfig,
    prelude::{AsyncFetch, Auth, PoolExtract, PostFetchAsync, PostFetchMethod, SinglePostFetch},
};
use ahash::HashMap;
use ibdl_common::{
    post::{extension::Extension, rating::Rating, Post},
    ImageBoards,
};
use log::{debug, error};
use reqwest::{Client, Method};
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

/// Defines the contract for site-specific API interactions, deserialization, and data mapping.
///
/// Implementors of this trait provide the necessary logic to communicate with a specific
/// imageboard's API, parse its responses, and convert them into the common `Post` format.
pub trait SiteApi: Send + Sync {
    /// The type that represents the direct deserialization of a list of posts
    /// from the imageboard's API (e.g., a JSON array of post objects).
    type PostListResponse: DeserializeOwned;
    /// The type that represents the direct deserialization of a single post
    /// from the imageboard's API.
    type SinglePostResponse: DeserializeOwned;
    /// The type that represents the direct deserialization of a pool's details,
    /// which typically includes a list of post IDs belonging to the pool.
    type PoolDetailsResponse: DeserializeOwned;

    /// Deserializes the raw string data from a post list API endpoint.
    fn deserialize_post_list(&self, data: &str) -> Result<Self::PostListResponse, ExtractorError>;

    /// Deserializes the raw string data from a single post API endpoint.
    ///
    /// # Arguments
    /// * `data`: A string slice containing the raw response body from the API.
    ///
    /// # Returns
    /// A `Result` containing the deserialized `PostListResponse` on success,
    /// or an `ExtractorError` if deserialization fails.
    fn deserialize_single_post(
        &self,
        data: &str,
    ) -> Result<Self::SinglePostResponse, ExtractorError>;

    /// Maps the deserialized API list response to a `Vec<Post>`.
    ///
    /// # Arguments
    /// * `data`: A string slice containing the raw response body from the API.
    ///
    /// # Returns
    /// A `Result` containing the deserialized `SinglePostResponse` on success,
    /// or an `ExtractorError` if deserialization fails or the operation is unsupported.
    fn map_post_list_response(
        &self,
        response: Self::PostListResponse,
    ) -> Result<Vec<Post>, ExtractorError>;

    /// Maps the deserialized API single post response to a `Post`.
    /// This method converts the site-specific `PostListResponse` into a vector
    /// of the common `Post` struct.
    ///
    /// # Arguments
    /// * `response`: The deserialized `PostListResponse` from the API.
    ///
    /// # Returns
    /// A `Result` containing a `Vec<Post>` on success, or an `ExtractorError` if mapping fails.
    fn map_single_post_response(
        &self,
        response: Self::SinglePostResponse,
    ) -> Result<Post, ExtractorError>;

    /// Gets the URL for a single post.
    ///
    /// # Arguments
    /// * `base_url`: The base URL for fetching single posts, typically from `ServerConfig.post_url`.
    /// * `post_id`: The ID of the post to fetch.
    ///
    /// # Returns
    /// A `String` representing the full URL to fetch the specified post.
    fn single_post_url(&self, base_url: &str, post_id: u32) -> String;

    /// Gets the URL for fetching a list of posts.
    ///
    /// # Arguments
    /// * `base_url`: The base URL for fetching post lists, typically from `ServerConfig.post_list_url`.
    /// * `page_num`: The page number to fetch (1-indexed).
    /// * `limit`: The requested number of posts per page.
    /// * `tags_query_string`: The already processed and formatted tag string suitable for the API query.
    ///
    /// # Returns
    /// A `String` representing the full URL to fetch the specified page of posts.
    fn posts_url(
        &self,
        base_url: &str,
        page_num: u16,
        limit: u16,
        tags_query_string: &str,
    ) -> String;

    /// Processes raw input tags into a format suitable for API queries and for internal use.
    ///
    /// This method is responsible for any site-specific tag transformations,
    /// such as joining tags into a query string, handling special tag syntax,
    /// or separating tags that might be used for different purposes (e.g., query vs. display).
    ///
    /// # Arguments
    /// * `input_tags`: A slice of strings, where each string is a raw tag provided by the user.
    ///
    /// # Returns
    /// A tuple `(String, Vec<String>)`:
    /// * The first element is a `String` formatted for use in the API query (e.g., "tag1+tag2+tag3").
    /// * The second element is a `Vec<String>` of tags intended for use in the `PostQueue` or for display,
    ///   which might be the same as input or processed differently.
    fn process_tags(&mut self, input_tags: &[String]) -> (String, Vec<String>);

    /// Returns the maximum number of pages the extractor should attempt to crawl
    /// during a `full_search` operation. This acts as a safeguard against
    /// excessively long or infinite searches.
    fn full_search_page_limit(&self) -> u16;

    /// Determines if the `full_search` loop should break based on the number
    /// of posts fetched on the current page. This is often used to stop searching
    /// when an API returns an empty page, indicating no more results.
    fn full_search_post_limit_break_condition(&self, posts_fetched_this_page: usize) -> bool;

    /// Optional delay between API calls in `full_search` loop.
    fn full_search_api_call_delay(&self) -> Option<Duration>;

    /// Specifies the delay to be used between API calls when fetching multiple
    /// individual posts (e.g., in `SinglePostFetch::get_posts`).
    fn multi_get_post_api_call_delay(&self) -> Duration;

    /// Returns the `ImageBoards` enum variant that corresponds to this site.
    fn imageboard_type(&self) -> ImageBoards;

    /// Returns the `ExtractorFeatures` supported by this site's API.
    fn features() -> ExtractorFeatures;

    /// Gets the URL for fetching pool details (which includes post IDs).
    ///
    /// # Arguments
    /// * `base_url`: The base URL for fetching pool details, typically from `ServerConfig.pool_idx_url`.
    /// * `pool_id`: The ID of the pool to fetch.
    ///
    /// # Returns
    /// A `String` representing the full URL to fetch the specified pool's details.
    fn pool_details_url(&self, base_url: &str, pool_id: u32) -> String;

    /// Deserializes the raw string data from a pool details API endpoint.
    ///
    /// # Arguments
    /// * `data`: A string slice containing the raw response body from the API.
    ///
    /// # Returns
    /// A `Result` containing the deserialized `PoolDetailsResponse` on success,
    /// or an `ExtractorError` if deserialization fails or the operation is unsupported.
    fn deserialize_pool_details_response(
        &self,
        data: &str,
    ) -> Result<Self::PoolDetailsResponse, ExtractorError>;

    /// Maps the deserialized pool details response to a `HashMap` of post IDs and their 0-based order index.
    /// The order index is crucial for preserving the sequence of posts within the pool.
    ///
    /// # Arguments
    /// * `response`: The deserialized `PoolDetailsResponse` from the API.
    ///
    /// # Returns
    /// A `Result` containing a `HashMap<u64, usize>` where keys are post IDs and values are their
    /// 0-indexed order within the pool. Returns an `ExtractorError` if mapping fails or the operation is unsupported.
    fn map_pool_details_to_post_ids_with_order(
        &self,
        response: Self::PoolDetailsResponse,
    ) -> Result<HashMap<u64, usize>, ExtractorError>;

    /// Parses a raw JSON string (presumably from a pool API) into a Vec of post IDs.
    /// The order of post IDs in the returned vector should reflect their order in the pool if specified by the API.
    ///
    /// # Arguments
    /// * `raw_json`: A string slice containing the raw JSON response.
    ///
    /// # Returns
    /// A `Result` containing a `Vec<u64>` of post IDs on success, or an `ExtractorError` if parsing fails
    /// or the operation is unsupported.
    fn parse_post_ids_from_pool_json_str(&self, raw_json: &str)
        -> Result<Vec<u64>, ExtractorError>;
}

/// A generic extractor for fetching and processing posts from an imageboard.
///
/// This struct encapsulates the common logic for interacting with an imageboard API,
/// including searching for posts, fetching individual posts, handling authentication,
/// applying blacklists, and managing pool downloads. It relies on a `SiteApi`
/// implementation for site-specific details.
#[derive(Debug, Clone)]
pub struct PostExtractor<S: SiteApi> {
    /// The HTTP client used for making requests to the imageboard.
    client: Client,
    /// Tags formatted specifically for the imageboard's API query.
    tags_for_query: String,
    /// Tags as a list of strings, often used for display or internal `PostQueue` logic.
    tags_for_post_queue: Vec<String>,
    /// The current authentication state with the imageboard.
    auth_state: AuthState,
    /// Configuration related to user authentication, including credentials and user-specific data.
    auth_config: ImageboardConfig,
    /// A list of `Rating`s to filter posts by; only posts with these ratings will be kept.
    download_ratings: Vec<Rating>,
    /// If `true`, all blacklist filtering (global, site-specific, auth) is disabled.
    disable_blacklist: bool,
    /// Counter for the total number of posts removed by all filters.
    total_removed: u64,
    /// If `true`, posts that are videos (or tagged as animated) will be included.
    /// If `false` (default for `ignore_animated` in `BlacklistFilter`), they will be filtered out.
    map_videos: bool,
    /// A list of tags provided by the user to be explicitly excluded from results.
    user_excluded_tags: Vec<String>,
    /// If `Some`, only posts with the specified `Extension` will be kept.
    selected_extension: Option<Extension>,
    /// Configuration specific to the imageboard server being accessed.
    server_cfg: ServerConfig,
    /// Optional configuration for downloading posts from a specific pool.
    pool_config: Option<PoolDownloadConfig>,
    /// The site-specific API handler.
    site_api: S,
}

/// Represents a collection of posts fetched from an imageboard.
///
/// This structure holds the posts themselves, along with context such as the
/// imageboard source, the client used for fetching, and the tags used for the search.
#[derive(Debug)]
pub struct PostQueue {
    /// The imageboard where the posts come from.
    pub imageboard: ImageBoards,
    /// The internal `Client` used by the extractor.
    pub client: Client,
    /// A list containing all `Post`s collected.
    pub posts: Vec<Post>,
    /// The tags used to search the collected posts.
    pub tags: Vec<String>,
}

impl PostQueue {
    /// Prepares the post queue, potentially limiting the number of posts.
    ///
    /// If `limit` is `Some`, the `posts` vector will be truncated to that size.
    /// If `limit` is `None`, the `posts` vector will be shrunk to fit its current
    /// content, potentially freeing up unused capacity.
    pub fn prepare(&mut self, limit: Option<u16>) {
        if let Some(max) = limit {
            self.posts.truncate(max as usize);
        } else {
            self.posts.shrink_to_fit();
        }
    }
}

/// Configuration for a pool-based download.
#[derive(Debug, Clone)]
struct PoolDownloadConfig {
    pool_id: u32,
    last_first: bool,
}

impl<S: SiteApi> PostExtractor<S> {
    /// Creates a new `PostExtractor` with the given site-specific API handler and configuration.
    ///
    /// This is the main constructor to be called by specific extractor initializers that provide
    /// their own `SiteApi` implementation and `ServerConfig`.
    ///
    /// # Arguments
    /// * `tags_raw`: A slice of raw tags (e.g., strings) to search for. These will be processed by `site_api.process_tags`.
    /// * `download_ratings`: A slice of `Rating` enum values. Only posts matching these ratings will be downloaded.
    /// * `disable_blacklist`: If `true`, all blacklist filtering will be disabled.
    /// * `map_videos`: If `true`, video posts (and posts tagged 'animated') will be included. If `false`, they will be filtered out
    ///   by the `BlacklistFilter` (as `ignore_animated` will be true).
    /// * `site_api`: An instance of a type implementing `SiteApi`, providing site-specific logic. This instance is consumed.
    /// * `server_cfg`: The `ServerConfig` for the imageboard being accessed.
    ///
    /// # Type Parameters
    /// * `T`: A type that can be converted to a `String` and implements `Display`, used for `tags_raw`.
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
        let client = Client::builder()
            .user_agent(&client_cfg.client_user_agent)
            .build()
            .unwrap();

        let initial_tags: Vec<String> = tags_raw
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        let (tags_for_query, tags_for_post_queue) = site_api.process_tags(&initial_tags);

        debug!("Processed Tags for API Query: {tags_for_query}");
        debug!("Tags for PostQueue: {tags_for_post_queue:?}");

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

    /// Fetches a single page of posts based on the configured tags and page number.
    ///
    /// The posts are sorted by ID in descending order before being returned in a `PostQueue`.
    ///
    /// # Arguments
    /// * `page`: The page number to fetch (1-indexed).
    ///
    /// # Returns
    /// A `Result` containing a `PostQueue` with the fetched posts on success, or an `ExtractorError` on failure (e.g., no posts found).
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

    /// Performs a search for posts across multiple pages,
    /// applying all configured filters (ratings, blacklist, extension, video).
    ///
    /// It handles pagination, rate limiting (via `SiteApi`), and stops when a limit
    /// is reached, no more posts are found, or a page limit is hit.
    ///
    /// # Arguments
    /// * `start_page`: An optional page number to begin the search from (1-indexed). Defaults to page 1 if `None`.
    /// * `limit`: An optional maximum number of posts to fetch. If `None`, fetches until other conditions are met.
    ///
    /// # Returns
    /// A `Result` containing a `PostQueue` with all fetched and filtered posts on success,
    /// or an `ExtractorError` on failure (e.g., no posts found after filtering, API errors).
    /// The posts in the queue are sorted by ID in descending order.
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
        debug!("Effective blacklist tags: {effective_blacklist_tags:?}");

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

            debug!("Scanning page {position}");

            let posts_this_page = self.get_post_list(position, limit).await?;
            let posts_count = posts_this_page.len();

            if self
                .site_api
                .full_search_post_limit_break_condition(posts_count)
                && page > 1
            // Don't break on first page if empty, get_post_list would error
            {
                debug!("Site API condition met to break search loop (posts: {posts_count}).");
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
                    debug!("Post limit ({num}) reached.");
                    break;
                }
            }

            if page >= page_limit {
                debug!("Page limit ({page_limit}) reached.");
                break;
            }

            page += 1;
            if let Some(delay) = self.site_api.full_search_api_call_delay() {
                debug!("Delaying for {delay:?} before next page.");
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

    /// Adds a list of tags to be excluded from search results.
    /// These tags are combined with blacklist tags during filtering.
    ///
    /// # Arguments
    /// * `tags`: A slice of `String`s representing the tags to exclude.
    ///
    /// # Returns
    /// A mutable reference to `self` for chaining.
    pub fn exclude_tags(&mut self, tags: &[String]) -> &mut Self {
        self.user_excluded_tags = tags.to_vec();
        self
    }

    /// Forces the extractor to only consider posts with a specific file extension.
    ///
    /// # Arguments
    /// * `extension`: The `Extension` to filter by.
    ///
    /// # Returns
    /// A mutable reference to `self` for chaining.
    pub fn force_extension(&mut self, extension: Extension) -> &mut Self {
        self.selected_extension = Some(extension);
        self
    }

    /// Fetches a list of posts for a specific page and optional limit.
    /// This method handles constructing the API URL, making the request (with authentication if configured),
    /// deserializing the response, and mapping it to `Vec<Post>`.
    ///
    /// # Arguments
    /// * `page`: The page number to fetch (1-indexed).
    /// * `limit_option`: An optional maximum number of posts to request for this page.
    ///   If `None`, `server_cfg.max_post_limit` is used. The actual limit is capped by `server_cfg.max_post_limit`.
    ///
    /// # Returns
    /// A `Result` containing a `Vec<Post>` on success, or an `ExtractorError` on failure.
    pub async fn get_post_list(
        &self,
        page: u16,
        limit_option: Option<u16>,
    ) -> Result<Vec<Post>, ExtractorError> {
        let base_post_url = self
            .server_cfg
            .post_list_url
            .as_ref()
            .ok_or_else(|| {
                debug!("Attempted to get post list, but 'post_list_url' is not configured for this server.");
                ExtractorError::UnsupportedOperation
            })?;

        let page_post_count = limit_option.map_or(self.server_cfg.max_post_limit, |count| {
            count.min(self.server_cfg.max_post_limit)
        });

        let final_url_string =
            self.site_api
                .posts_url(base_post_url, page, page_post_count, &self.tags_for_query);

        let mut request_builder = self.client.request(Method::GET, &final_url_string);

        if self.auth_state.is_auth() {
            debug!("[AUTH] Fetching posts from page {page} via URL: {final_url_string}");
            request_builder = request_builder
                .basic_auth(&self.auth_config.username, Some(&self.auth_config.api_key));
        } else {
            debug!("Fetching posts from page {page} via URL: {final_url_string}");
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

    /// Returns a clone of the internal HTTP client.
    pub fn client(&self) -> Client {
        self.client.clone()
    }

    /// Returns the total number of posts removed by filtering operations so far.
    pub const fn total_removed(&self) -> u64 {
        self.total_removed
    }

    /// Returns the `ImageBoards` enum variant for the current extractor.
    pub fn imageboard(&self) -> ImageBoards {
        self.site_api.imageboard_type()
    }

    /// Returns the `ExtractorFeatures` supported by the underlying `SiteApi`.
    #[must_use]
    pub fn features() -> ExtractorFeatures {
        S::features()
    }

    /// Returns a clone of the `ServerConfig` used by this extractor.
    pub fn config(&self) -> ServerConfig {
        self.server_cfg.clone()
    }
}

impl<S: SiteApi> Auth for PostExtractor<S> {
    /// Sets the authentication state and configuration for the extractor.
    ///
    /// After calling this, subsequent API requests that support authentication
    /// will use the provided credentials. Blacklisted tags from the authenticated
    /// user's profile may also be incorporated into the filtering process.
    ///
    /// # Arguments
    /// * `config`: An `ImageboardConfig` containing the username, API key, and
    ///   other user-specific data (like blacklisted tags).
    ///
    /// # Returns
    /// `Ok(())` on success, or an `ExtractorError` if an issue occurs (though typically this method is infallible).
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
    /// Maps raw string data (presumably a single post's API response) to a `Post` struct.
    /// This typically involves deserializing the data using `SiteApi::deserialize_single_post`
    /// and then mapping it using `SiteApi::map_single_post_response`.
    ///
    /// # Arguments
    /// * `raw_data`: A `String` containing the raw API response for a single post.
    ///
    /// # Returns
    /// A `Result` containing the mapped `Post` on success, or an `ExtractorError` on failure.
    fn map_post(&self, raw_data: String) -> Result<Post, ExtractorError> {
        let deserialized_post = self.site_api.deserialize_single_post(&raw_data)?;
        self.site_api.map_single_post_response(deserialized_post)
    }

    /// Fetches a single post by its ID.
    ///
    /// Handles URL construction, authentication (if configured), making the API request,
    /// deserializing the response, and mapping it to a `Post`.
    ///
    /// # Arguments
    /// * `post_id`: The ID of the post to fetch.
    ///
    /// # Returns
    /// A `Result` containing the fetched `Post` on success, or an `ExtractorError` on failure.
    async fn get_post(&mut self, post_id: u32) -> Result<Post, ExtractorError> {
        let base_url = self.server_cfg.post_url.as_ref().ok_or_else(|| {
            debug!(
                "Attempted to get single post, but 'post_url' is not configured for this server."
            );
            ExtractorError::UnsupportedOperation
        })?;
        let url = self.site_api.single_post_url(base_url, post_id);

        let mut request_builder = self.client.get(url);
        if self.auth_state.is_auth() {
            debug!("[AUTH] Fetching post {post_id}");
            request_builder = request_builder
                .basic_auth(&self.auth_config.username, Some(&self.auth_config.api_key));
        } else {
            debug!("Fetching post {post_id}");
        }

        let raw_body = request_builder.send().await?.text().await?;
        let parsed_response = self.site_api.deserialize_single_post(&raw_body)?;

        let start_point = Instant::now();
        let post = self.site_api.map_single_post_response(parsed_response)?;
        debug!("Single post mapping took {:?}", start_point.elapsed());
        Ok(post)
    }

    /// Fetches multiple posts by their IDs.
    ///
    /// Iterates through the provided IDs, calling `get_post` for each.
    /// Includes a delay between API calls as specified by `SiteApi::multi_get_post_api_call_delay`.
    ///
    /// # Arguments
    /// * `posts_ids`: A slice of `u32` post IDs to fetch.
    ///
    /// # Returns
    /// A `Result` containing a `Vec<Post>` of the fetched posts on success, or an `ExtractorError` if any fetch fails.
    async fn get_posts(&mut self, posts_ids: &[u32]) -> Result<Vec<Post>, ExtractorError> {
        let mut pvec = Vec::with_capacity(posts_ids.len());
        let delay = self.site_api.multi_get_post_api_call_delay();

        for (idx, post_id) in posts_ids.iter().enumerate() {
            let post = self.get_post(*post_id).await?;
            pvec.push(post);

            if idx < posts_ids.len() - 1 {
                debug!("Debouncing API calls by {delay:?}");
                sleep(delay).await;
            }
        }
        Ok(pvec)
    }
}

impl<S: SiteApi + 'static> PostExtractor<S> {
    /// Internal logic for asynchronously fetching posts from a configured pool.
    ///
    /// This method is called by `AsyncFetch::async_fetch` when `pool_config` is `Some`.
    /// It fetches all post IDs for the pool, applies ordering and limits, then fetches
    /// each post individually, filters it, and sends it through the `sender_channel`.
    ///
    /// # Arguments
    /// * `sender_channel`: An unbounded sender to send fetched `Post`s to.
    /// * `limit`: An optional overall limit on the number of posts to fetch from the pool.
    /// * `post_counter`: An optional sender to report the count of posts successfully sent.
    /// * `pool_cfg`: The `PoolDownloadConfig` specifying the pool ID and ordering.
    /// # Returns
    /// A `Result` containing the total number of posts removed by the blacklist during this operation, or an `ExtractorError`.
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
                        debug!("Post ID {post_id_u64} from pool was filtered out.");
                    }
                }
                Err(e) => {
                    error!("Failed to fetch post ID {post_id_u64} from pool: {e:?}");
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

    /// Internal logic for asynchronously fetching posts based on tags.
    ///
    /// This method is called by `AsyncFetch::async_fetch` when `pool_config` is `None`.
    /// It paginates through API results, fetches lists of posts, applies filters (including
    /// positive tag matching and blacklisting), and sends qualifying posts through the `sender_channel`.
    ///
    /// # Arguments
    /// * `sender_channel`: An unbounded sender to send fetched `Post`s to.
    /// * `start_page`: An optional page number to begin the search from (1-indexed).
    /// * `limit`: An optional overall limit on the number of posts to fetch.
    /// * `post_counter`: An optional sender to report the count of posts successfully sent.
    /// # Returns
    /// A `Result` containing the total number of posts removed by the blacklist during this operation, or an `ExtractorError`.
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
                debug!("No more posts found on page {position}.");
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
                    debug!("Target post count of {num} reached.");
                    break;
                }
            }

            if page >= page_limit {
                debug!("Max number of pages ({page_limit}) reached");
                break;
            }

            page += 1;
            if let Some(delay) = self.site_api.full_search_api_call_delay() {
                debug!("Delaying for {delay:?} before next page.");
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
    /// Asynchronously fetches posts and sends them through a channel.
    ///
    /// This method serves as the main entry point for asynchronous fetching.
    /// It delegates to either `async_fetch_pool_logic` if a pool download is configured,
    /// or `async_fetch_tag_logic` for tag-based searches.
    ///
    /// # Arguments
    /// * `sender_channel`: An unbounded sender to send fetched `Post`s to.
    /// * `start_page`: An optional page number to begin the search from (1-indexed). Used for tag-based search.
    /// * `limit`: An optional overall limit on the number of posts to fetch.
    /// * `post_counter`: An optional sender to report the count of posts successfully sent.
    /// # Returns
    /// A `Result` containing the total number of posts removed by the blacklist during this operation, or an `ExtractorError`.
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

    /// Spawns a new Tokio task to perform the asynchronous fetch operation.
    ///
    /// # Arguments
    /// * `self`: Consumes the `PostExtractor` instance.
    /// * `sender_channel`: An unbounded sender to send fetched `Post`s to.
    /// * `start_page`: An optional page number to begin the search from (1-indexed).
    /// * `limit`: An optional overall limit on the number of posts to fetch.
    /// * `post_counter`: An optional sender to report the count of posts successfully sent.
    ///
    /// # Returns
    /// A `JoinHandle` for the spawned task, which will resolve to the result of `async_fetch`.
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
    /// Spawns a new Tokio task to asynchronously fetch one or more specific posts by their IDs.
    ///
    /// # Arguments
    /// * `self`: Consumes the `PostExtractor` instance.
    /// * `post_channel`: An unbounded sender to send the fetched `Post`(s) to.
    /// * `method`: A `PostFetchMethod` enum specifying whether to fetch a single post or multiple posts by their IDs.
    /// * `length_channel`: A sender to report the number of posts successfully fetched and sent.
    ///
    /// # Returns
    /// A `JoinHandle` for the spawned task. The task result is `Ok(0)` on successful completion of sending posts,
    /// or an `ExtractorError` if any operation fails. The `0` indicates that `total_removed` is not
    /// directly applicable in this context as it is for broader searches.
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
                            debug!("Debouncing API calls by {delay:?}");
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
    /// Fetches the post IDs and their order within a specific pool.
    ///
    /// Handles URL construction, authentication, API request, deserialization,
    /// and mapping the response to a `HashMap` where keys are post IDs and
    /// values are their 0-indexed order in the pool. An optional limit can be applied.
    ///
    /// # Arguments
    /// * `pool_id`: The ID of the pool to fetch.
    /// * `limit_opt`: An optional limit on the number of post IDs to retrieve from the pool.
    ///
    /// # Returns
    /// A `Result` containing a `HashMap<u64, usize>` of post IDs and their order on success, or an `ExtractorError`.
    async fn fetch_pool_idxs(
        &mut self,
        pool_id: u32,
        limit_opt: Option<u16>,
    ) -> Result<HashMap<u64, usize>, ExtractorError> {
        let base_url = self
            .server_cfg
            .pool_idx_url
            .as_ref()
            .ok_or_else(|| {
                debug!("Attempted to fetch pool indexes, but 'pool_idx_url' is not configured for this server.");
                ExtractorError::UnsupportedOperation
            })?;

        let url = self.site_api.pool_details_url(base_url, pool_id);
        debug!("Fetching pool details from URL: {url}");

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

    /// Parses a raw JSON string, assumed to be from a pool API endpoint, into a `Vec<u64>` of post IDs.
    /// The order of IDs in the vector should reflect their order in the pool if implied by the JSON structure.
    ///
    /// # Arguments
    /// * `raw_json`: A `String` containing the raw JSON data.
    ///
    /// # Returns
    /// A `Result` containing a `Vec<u64>` of post IDs on success, or an `ExtractorError` if parsing fails.
    fn parse_pool_ids(&self, raw_json: String) -> Result<Vec<u64>, ExtractorError> {
        self.site_api.parse_post_ids_from_pool_json_str(&raw_json)
    }

    /// Configures the `PostExtractor` for a pool download operation or clears existing pool configuration.
    ///
    /// # Arguments
    /// * `pool_id`: An `Option<u32>`. If `Some(id)`, the extractor is configured to download pool `id`.
    ///   If `None`, any existing pool download configuration is cleared, and the extractor will perform tag-based searches.
    /// * `last_first`: If `true` and `pool_id` is `Some`, posts from the pool will be processed in reverse order.
    fn setup_pool_download(&mut self, pool_id: Option<u32>, last_first: bool) {
        if let Some(id) = pool_id {
            self.pool_config = Some(PoolDownloadConfig {
                pool_id: id,
                last_first,
            });
            debug!("PostExtractor configured for pool download: ID {id}, last_first: {last_first}");
        } else {
            self.pool_config = None;
            debug!("PostExtractor pool download configuration cleared.");
        }
    }
}
