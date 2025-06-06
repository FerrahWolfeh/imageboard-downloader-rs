//! Post extractor for Gelbooru-based imageboards
//!
//! This extractor is compatible with these imageboards:
//! * `Imageboards::Rule34`
//! * `Imageboards::Realbooru`
//! * `Imageboards::Gelbooru`
//!
//!

// NOTE: https://gelbooru.com/index.php?page=dapi&s=tag&q=index&json=1&names=folinic_(arknights)%20arknights%20ru_zhai%20highres%20black_hair
// This is to search all tags and their meanings.
// I've to do an enum based on this thing.

use ahash::HashMap;
use ibdl_common::post::extension::Extension;
use ibdl_common::{
    extract_ext_from_url,
    post::{rating::Rating, Post},
    ImageBoards,
};
use log::debug;
use serde_json;
use std::time::Duration;

use crate::error::ExtractorError;
use crate::extractor::caps::ExtractorFeatures;
use crate::extractor::SiteApi;
use crate::imageboards::gelbooru::models::GelbooruTopLevel;

// mod gelbooru_old;
mod models;
// mod unsync;

// Define the GelbooruApi struct
pub struct GelbooruApi;

impl GelbooruApi {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for GelbooruApi {
    fn default() -> Self {
        Self::new()
    }
}

// Implement the SiteApi trait for GelbooruApi
impl SiteApi for GelbooruApi {
    type PostListResponse = GelbooruTopLevel;
    type SinglePostResponse = GelbooruTopLevel; // Gelbooru single post endpoint returns a list
    type PoolDetailsResponse = (); // Gelbooru standard API doesn't support pools

    fn deserialize_post_list(&self, data: &str) -> Result<Self::PostListResponse, ExtractorError> {
        let response = serde_json::from_str::<GelbooruTopLevel>(data)?;
        Ok(response)
    }

    fn deserialize_single_post(
        &self,
        data: &str,
    ) -> Result<Self::SinglePostResponse, ExtractorError> {
        self.deserialize_post_list(data)
    }

    fn map_post_list_response(
        &self,
        response: Self::PostListResponse,
    ) -> Result<Vec<Post>, ExtractorError> {
        let batch = response.post.into_iter().filter(|c| c.file_url.is_some());

        let mapper_iter = batch.map(|c| {
            let tag_list = c.map_tags();

            let rt = c.rating.unwrap_or_else(|| "s".to_string()); // Default to safe if rating is missing
            let rating = Rating::from_rating_str(&rt);
            let xt = c.file_url.unwrap(); // Filtered out None already

            let extension = extract_ext_from_url!(xt);

            Post {
                id: c.id.unwrap_or(0), // Default to 0 if ID is missing (shouldn't happen)
                website: ImageBoards::Gelbooru,
                md5: c.md5.unwrap_or_else(|| "unknown".to_string()), // Default if MD5 is missing
                url: xt,
                extension: Extension::guess_format(&extension),
                tags: tag_list,
                rating,
            }
        });

        Ok(mapper_iter.collect::<Vec<Post>>())
    }

    fn map_single_post_response(
        &self,
        response: Self::SinglePostResponse,
    ) -> Result<Post, ExtractorError> {
        // Expect the response to contain exactly one post in the 'post' vector
        let mut posts = self.map_post_list_response(response)?;
        if posts.len() == 1 {
            Ok(posts.remove(0))
        } else if posts.is_empty() {
            Err(ExtractorError::ZeroPosts)
        } else {
            // This shouldn't happen for a single post ID query, but handle defensively
            Err(ExtractorError::InvalidServerResponse)
        }
    }

    fn single_post_url(&self, base_url: &str, post_id: u32) -> String {
        // Use the standard Gelbooru DAPI single post endpoint
        format!("{base_url}?page=dapi&s=post&q=index&json=1&id={post_id}")
    }

    fn posts_url(
        &self,
        base_url_with_query: &str,
        page_num: u16,
        limit: u16,
        tags_query_string: &str,
    ) -> String {
        // Gelbooru DAPI uses 'pid' for page number, which is 0-indexed.
        // page_num is 1-indexed as passed from PostExtractor.
        let pid = if page_num > 0 { page_num - 1 } else { 0 };

        // base_url_with_query is expected to be like "https://gelbooru.com/index.php?page=dapi&s=post&q=index&json=1"
        // So, additional parameters are appended with '&'.
        let mut url = format!("{base_url_with_query}&limit={limit}&pid={pid}");

        if !tags_query_string.is_empty() {
            url.push_str("&tags=");
            url.push_str(tags_query_string);
        }
        url
    }

    fn process_tags(&mut self, input_tags: &[String]) -> (String, Vec<String>) {
        // Gelbooru uses '+' to separate tags in the query string
        let tag_string = input_tags.join("+");
        (tag_string, input_tags.to_vec()) // Return both query string and vector
    }

    fn full_search_page_limit(&self) -> u16 {
        // Based on the old implementation's loop condition
        100
    }

    fn full_search_post_limit_break_condition(&self, posts_fetched_this_page: usize) -> bool {
        // Break if no posts were found on the current page
        posts_fetched_this_page == 0
    }

    fn full_search_api_call_delay(&self) -> Option<Duration> {
        // Based on the old implementation's debounce
        Some(Duration::from_millis(500))
    }

    fn multi_get_post_api_call_delay(&self) -> Duration {
        // Based on the old implementation's debounce
        Duration::from_millis(500)
    }

    fn imageboard_type(&self) -> ImageBoards {
        ImageBoards::Gelbooru
    }

    fn features() -> ExtractorFeatures {
        // Gelbooru supports TagSearch, AsyncFetch, and SinglePostFetch (via its API)
        // It does NOT support PoolExtract or Auth (for blacklisted tags via API)
        ExtractorFeatures::from_bits_truncate(
            ExtractorFeatures::AsyncFetch.bits()
                | ExtractorFeatures::TagSearch.bits()
                | ExtractorFeatures::SinglePostFetch.bits(),
        )
    }

    // Pool methods (unsupported for standard Gelbooru API)
    fn pool_details_url(&self, _base_url: &str, _pool_id: u32) -> String {
        // This should ideally not be called if features() is correct, but provide a placeholder
        debug!("Attempted to call pool_details_url on GelbooruApi, which does not support pools.");
        String::new() // Return empty string, the caller should handle the UnsupportedOperation error
    }

    fn deserialize_pool_details_response(
        &self,
        _data: &str,
    ) -> Result<Self::PoolDetailsResponse, ExtractorError> {
        Err(ExtractorError::UnsupportedOperation)
    }

    fn map_pool_details_to_post_ids_with_order(
        &self,
        _response: Self::PoolDetailsResponse,
    ) -> Result<HashMap<u64, usize>, ExtractorError> {
        Err(ExtractorError::UnsupportedOperation)
    }

    fn parse_post_ids_from_pool_json_str(
        &self,
        _raw_json: &str,
    ) -> Result<Vec<u64>, ExtractorError> {
        Err(ExtractorError::UnsupportedOperation)
    }
}

#[cfg(test)]
mod test {
    use ibdl_common::{post::rating::Rating, ImageBoards};
    use tokio::{join, sync::mpsc::unbounded_channel};

    use crate::{
        blacklist::{GlobalBlacklist, DEFAULT_BLACKLIST_TOML},
        extractor::PostExtractor,
        extractor_config::DEFAULT_SERVERS,
        imageboards::{danbooru::DanbooruApi, prelude::GelbooruApi},
        prelude::AsyncFetch,
    };

    #[tokio::test]
    async fn post_api() {
        let server_config = DEFAULT_SERVERS.get("gelbooru").unwrap().clone();

        let api = GelbooruApi::new();

        let global_blacklist = GlobalBlacklist::from_config(DEFAULT_BLACKLIST_TOML).unwrap();

        let extractor = PostExtractor::new(
            &["1girl", "cyrene_(honkai:_star_rail)"],
            &global_blacklist,
            &[], // ratings_to_download
            false,
            false,
            api, // Pass the DanbooruApi instance
            server_config,
        );

        let post_list = extractor.get_post_list(1, None).await;
        // Assertions to check the content of the parsed post list.
        assert!(
            post_list.is_ok(),
            "Failed to fetch post list: {:?}",
            post_list.err()
        );

        let posts = post_list.unwrap();
        assert!(!posts.is_empty(), "Post list is empty");

        // Check some properties of the first post.
        let first_post = &posts[0];
        assert_eq!(first_post.website, ImageBoards::Gelbooru);
        assert!(!first_post.md5.is_empty());
        assert!(!first_post.url.is_empty());
        assert_ne!(first_post.rating, Rating::Unknown);
        assert!(first_post.tags.iter().any(|tag| tag.tag() == "1girl"));
    }

    #[tokio::test]
    async fn async_fetch() {
        let server_config = DEFAULT_SERVERS.get("danbooru").unwrap().clone();
        let danbooru_api = DanbooruApi::new();

        // Tags for the test. DanbooruApi will use the first two for the API query,
        // and all of them for tags_for_post_queue (used in positive filtering by async_fetch).
        let tags_to_search = &["1girl", "touhou"];
        let ratings_to_download = &[]; // No specific rating filter for this test
        let disable_blacklist = true; // Disable blacklist for simplicity in this test
        let map_videos = false;

        let global_blacklist = GlobalBlacklist::from_config(DEFAULT_BLACKLIST_TOML).unwrap();

        let extractor = PostExtractor::new(
            tags_to_search,
            &global_blacklist,
            ratings_to_download,
            disable_blacklist,
            map_videos,
            danbooru_api,
            server_config,
        );

        let (post_sender, mut post_receiver) = unbounded_channel();
        let post_limit = Some(100u16); // We expect to fetch 5 posts

        // setup_fetch_thread consumes the extractor
        let fetch_handle = extractor.setup_fetch_thread(post_sender, None, post_limit, None);

        let mut received_posts = Vec::new();
        // Collect posts until the channel is closed (sender is dropped) or limit is reached
        while let Some(post) = post_receiver.recv().await {
            received_posts.push(post);
        }

        // Ensure the fetch thread completed successfully
        let fetch_result = join!(fetch_handle).0.unwrap();
        assert!(
            fetch_result.is_ok(),
            "Async fetch failed: {:?}",
            fetch_result.err()
        );

        let total_removed_by_blacklist = fetch_result.unwrap();
        // Since blacklist is disabled and no ratings are specified to filter out,
        // no posts should be removed by the blacklist logic in async_fetch.
        assert_eq!(
            total_removed_by_blacklist, 0,
            "Blacklist removed posts unexpectedly, expected 0 with disable_blacklist=true."
        );

        assert_eq!(
            received_posts.len(),
            100,
            "Did not receive the expected number of posts"
        );

        for post in received_posts {
            assert_eq!(post.website, ImageBoards::Danbooru);
            assert!(!post.md5.is_empty());
            assert!(!post.url.is_empty());
            assert_ne!(post.rating, Rating::Unknown);
            // Check that posts contain all the searched tags (due to positive filtering in async_fetch)
            for tag_str in tags_to_search {
                assert!(
                    post.tags.iter().any(|t| t.tag() == *tag_str),
                    "Post ID {} is missing tag '{}'",
                    post.id,
                    tag_str
                );
            }
        }
    }
}
