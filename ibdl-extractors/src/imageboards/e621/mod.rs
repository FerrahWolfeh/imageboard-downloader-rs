//! Post extractor for `https://e621.net`
//!
//! The e621 extractor has the following features:
//! - Authentication
//! - Native blacklist (defined in user profile page)
use ahash::HashMap;
use ibdl_common::post::extension::Extension;
use ibdl_common::serde_json;
use ibdl_common::{
    join_tags,
    post::{rating::Rating, Post},
    ImageBoards,
};
use std::time::Duration;

use crate::extractor::caps::ExtractorFeatures;
use crate::extractor::SiteApi;
use crate::{
    error::ExtractorError,
    imageboards::e621::models::{E621PoolList, E621SinglePostTopLevel, E621TopLevel},
};
use ibdl_common::log::debug;

mod models;

//const _E621_FAVORITES: &str = "https://e621.net/favorites.json";

/// API logic for E621.
#[derive(Debug, Clone, Copy, Default)]
pub struct E621Api;

impl E621Api {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl SiteApi for E621Api {
    type PostListResponse = E621TopLevel;
    type SinglePostResponse = E621SinglePostTopLevel;
    type PoolDetailsResponse = E621PoolList;

    fn deserialize_post_list(&self, data: &str) -> Result<Self::PostListResponse, ExtractorError> {
        let items: E621TopLevel = serde_json::from_str(data)?;
        Ok(items)
    }

    fn deserialize_single_post(
        &self,
        data: &str,
    ) -> Result<Self::SinglePostResponse, ExtractorError> {
        let c: E621SinglePostTopLevel = serde_json::from_str::<E621SinglePostTopLevel>(data)?;
        Ok(c)
    }

    fn map_post_list_response(
        &self,
        response: Self::PostListResponse,
    ) -> Result<Vec<Post>, ExtractorError> {
        let mut posts = Vec::with_capacity(response.posts.len());
        for item in response.posts {
            if item.file.url.is_none() || item.id.is_none() || item.file.md5.is_none() {
                debug!("Skipping post due to missing essential data: {item:?}");
                continue;
            }

            let tag_list = item.tags.map_tags();

            posts.push(Post {
                id: item.id.unwrap(), // Safe due to check above
                website: ImageBoards::E621,
                url: item.file.url.clone().unwrap(), // Safe due to check above
                md5: item.file.md5.clone().unwrap(), // Safe due to check above
                extension: Extension::guess_format(&item.file.ext.clone().unwrap_or_default()),
                tags: tag_list,
                rating: Rating::from_rating_str(&item.rating),
            });
        }
        Ok(posts)
    }

    fn map_single_post_response(
        &self,
        response: Self::SinglePostResponse,
    ) -> Result<Post, ExtractorError> {
        if response.post.file.url.is_none()
            || response.post.id.is_none()
            || response.post.file.md5.is_none()
        {
            return Err(ExtractorError::PostMapFailure);
        }

        // All unwraps below are safe due to the check above
        Ok(Post {
            id: response.post.id.unwrap(),
            website: ImageBoards::E621,
            url: response.post.file.url.clone().unwrap(),
            md5: response.post.file.md5.clone().unwrap(),
            extension: Extension::guess_format(&response.post.file.ext.clone().unwrap_or_default()),
            tags: response.post.tags.map_tags(),
            rating: Rating::from_rating_str(&response.post.rating),
        })
    }

    fn single_post_url(&self, base_url: &str, post_id: u32) -> String {
        format!("{base_url}/{post_id}.json")
    }

    fn process_tags(&mut self, input_tags: &[String]) -> (String, Vec<String>) {
        let tags_for_api = join_tags!(input_tags.iter().map(AsRef::as_ref).collect::<Vec<&str>>());
        (tags_for_api, input_tags.to_vec())
    }

    fn full_search_page_limit(&self) -> u16 {
        // e621 API: page parameter has a maximum of 750 for unauthenticated users
        750
    }

    fn full_search_post_limit_break_condition(&self, posts_fetched_this_page: usize) -> bool {
        // e621's max posts per page is 320. If less, it's likely the last page.
        posts_fetched_this_page < 320
    }

    fn full_search_api_call_delay(&self) -> Option<Duration> {
        Some(Duration::from_millis(500)) // Standard e621 debounce
    }

    fn multi_get_post_api_call_delay(&self) -> Duration {
        Duration::from_millis(500) // Standard e621 debounce
    }

    fn imageboard_type(&self) -> ImageBoards {
        ImageBoards::E621
    }

    fn features() -> ExtractorFeatures {
        ExtractorFeatures::from_bits_truncate(0b0001_1111) // AsyncFetch + TagSearch + SinglePostDownload + PoolDownload + Auth
    }

    fn pool_details_url(&self, base_url: &str, pool_id: u32) -> String {
        // base_url is ServerConfig.pool_idx_url
        format!("{base_url}/{pool_id}.json")
    }

    fn deserialize_pool_details_response(
        &self,
        data: &str,
    ) -> Result<Self::PoolDetailsResponse, ExtractorError> {
        serde_json::from_str(data).map_err(ExtractorError::JsonSerializeFail)
    }

    fn map_pool_details_to_post_ids_with_order(
        &self,
        response: Self::PoolDetailsResponse,
    ) -> Result<HashMap<u64, usize>, ExtractorError> {
        Ok(response
            .post_ids
            .into_iter()
            .enumerate()
            .map(|(index, id)| (id, index))
            .collect())
    }

    fn parse_post_ids_from_pool_json_str(
        &self,
        raw_json: &str,
    ) -> Result<Vec<u64>, ExtractorError> {
        let list: E621PoolList =
            serde_json::from_str(raw_json).map_err(ExtractorError::JsonSerializeFail)?;
        Ok(list.post_ids)
    }

    fn posts_url(
        &self,
        base_url: &str,
        page_num: u16,
        limit: u16,
        tags_query_string: &str,
    ) -> String {
        // Danbooru uses 'page' (1-indexed) and 'limit'.
        // base_url is typically "https://e621.net/posts.json"
        // It doesn't have query parameters yet, so start with '?'
        let mut url = format!("{base_url}?limit={limit}&page={page_num}");

        if !tags_query_string.is_empty() {
            url.push_str("&tags=");
            url.push_str(tags_query_string);
        }
        url
    }
}

#[cfg(test)]
mod test {
    use ahash::{HashMap, HashMapExt};
    use ibdl_common::{post::rating::Rating, ImageBoards};
    use tokio::{join, sync::mpsc::unbounded_channel};

    use crate::{
        extractor::PostExtractor,
        extractor_config::DEFAULT_SERVERS,
        imageboards::e621::E621Api,
        prelude::{AsyncFetch, PoolExtract},
    };

    #[tokio::test]
    async fn post_api() {
        let server_config = DEFAULT_SERVERS.get("e621").unwrap().clone();

        let e621_api = E621Api::new();

        let extractor = PostExtractor::new(
            &["feral", "canine"], // Use e621-relevant tags
            &[],
            false,
            false,
            e621_api, // Pass the E621Api instance
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
        assert_eq!(first_post.website, ImageBoards::E621);
        assert!(!first_post.md5.is_empty());
        assert!(!first_post.url.is_empty());
        assert_ne!(first_post.rating, Rating::Unknown);
        // Check that at least one of the searched tags is present (get_post_list doesn't positive filter)
        assert!(
            first_post.tags.iter().any(|tag| tag.tag() == "feral")
                || first_post.tags.iter().any(|tag| tag.tag() == "canine")
        );
    }

    #[tokio::test]
    async fn async_fetch() {
        let server_config = DEFAULT_SERVERS.get("e621").unwrap().clone();
        let e621_api = E621Api::new();

        // Tags for the test. E621Api will use all of them for the API query.
        // async_fetch will then positive filter to ensure all tags are present.
        let tags_to_search = &["feral", "canine", "male"];
        let ratings_to_download = &[]; // No specific rating filter for this test
        let disable_blacklist = true; // Disable blacklist for simplicity in this test
        let map_videos = false;

        let extractor = PostExtractor::new(
            tags_to_search,
            ratings_to_download,
            disable_blacklist,
            map_videos,
            e621_api,
            server_config,
        );

        let (post_sender, mut post_receiver) = unbounded_channel();
        let post_limit = Some(100u16); // We expect to fetch up to 100 posts

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
            assert_eq!(post.website, ImageBoards::E621);
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

    #[tokio::test]
    async fn pool_download_fetch_idxs() {
        // 1. Setup: Get ServerConfig for e621
        let server_config = DEFAULT_SERVERS
            .get("e621")
            .expect("e621 server config not found")
            .clone();

        assert!(
            server_config.pool_idx_url.is_some(),
            "e621 server config is missing 'pool_idx_url'"
        );

        // 2. Create E621Api
        let e621_api = E621Api::new();

        // 3. Create PostExtractor
        // Tags, ratings, etc., are not strictly needed for fetch_pool_idxs itself,
        // but PostExtractor requires them for initialization.
        let mut extractor = PostExtractor::new(
            &Vec::<String>::new(), // No tags needed for this specific pool fetch
            &[],                   // No ratings needed
            true,                  // disable_blacklist
            false,                 // map_videos
            e621_api,
            server_config,
        );

        // 4. Define pool ID and expected posts for a known small e621 pool
        // Pool #27455 "Test Pool" has 3 posts: [9415510, 9410674, 9410251]
        let pool_id_to_test = 18135; // Updated pool ID
        let expected_post_ids_in_order: Vec<u64> = vec![1_972_824, 1_972_825]; // Updated post IDs

        // 5. Call fetch_pool_idxs
        let result = extractor.fetch_pool_idxs(pool_id_to_test, None).await;

        // 6. Assertions for the full pool
        assert!(result.is_ok(), "fetch_pool_idxs failed: {:?}", result.err());
        let pool_map = result.unwrap();

        assert_eq!(
            pool_map.len(),
            expected_post_ids_in_order.len(),
            "Mismatch in number of posts in the pool"
        );

        for (expected_idx, expected_post_id) in expected_post_ids_in_order.iter().enumerate() {
            assert_eq!(
                pool_map.get(expected_post_id),
                Some(&expected_idx),
                "Post ID {} not found or has incorrect order in pool map. Expected order: {}, Got: {:?}",
                expected_post_id, expected_idx, pool_map.get(expected_post_id)
            );
        }

        // 7. Test with limit
        let limit = 1u16; // Adjusted limit to be less than total posts in new pool
        let result_limited = extractor
            .fetch_pool_idxs(pool_id_to_test, Some(limit))
            .await;
        assert!(
            result_limited.is_ok(),
            "fetch_pool_idxs with limit failed: {:?}",
            result_limited.err()
        );
        let pool_map_limited = result_limited.unwrap();

        assert_eq!(
            pool_map_limited.len(),
            limit as usize,
            "Mismatch in number of posts with limit"
        );

        // Construct the expected map for the limited case
        let mut expected_limited_map = HashMap::with_capacity(limit as usize);
        expected_post_ids_in_order
            .iter()
            .enumerate()
            .take(limit as usize)
            .for_each(|(idx, _)| {
                expected_limited_map.insert(expected_post_ids_in_order[idx], idx);
            });

        assert_eq!(
            pool_map_limited, expected_limited_map,
            "Limited pool map does not match expected content and order."
        );
    }

    #[tokio::test]
    async fn pool_async_fetch() {
        let server_config = DEFAULT_SERVERS
            .get("e621")
            .expect("e621 server config not found")
            .clone();

        assert!(
            server_config.pool_idx_url.is_some(),
            "e621 server config is missing 'pool_idx_url'"
        );

        let pool_id_to_test = 18135; // Updated pool ID
        let all_expected_post_ids_in_order: Vec<u64> = vec![1_972_824, 1_972_825]; // Updated post IDs

        // Test Case 1: Limit 2, last_first = false
        let mut extractor1 = PostExtractor::new(
            &Vec::<String>::new(), // No tags for pool download
            &[],                   // No specific ratings
            true,                  // disable_blacklist
            false,                 // map_videos
            E621Api::new(),
            server_config.clone(),
        );
        extractor1.setup_pool_download(Some(pool_id_to_test), false); // last_first = false

        let (post_sender1, mut post_receiver1) = unbounded_channel();
        let limit1 = 2u16; // This will fetch all posts from the new pool

        let fetch_handle1 = extractor1.setup_fetch_thread(post_sender1, None, Some(limit1), None);

        let mut received_posts1 = Vec::new();
        while let Some(post) = post_receiver1.recv().await {
            received_posts1.push(post);
        }
        join!(fetch_handle1).0.unwrap().unwrap(); // Ensure thread finished ok

        assert_eq!(
            received_posts1.len(),
            limit1 as usize,
            "Test Case 1: Incorrect number of posts received"
        );
        for post in received_posts1 {
            assert_eq!(
                post.website,
                ImageBoards::E621,
                "Test Case 1: Post from incorrect website"
            );
        }

        // Test Case 2: Limit 1, last_first = true
        let mut extractor2 = PostExtractor::new(
            &Vec::<String>::new(),
            &[],
            true,
            false,
            E621Api::new(),
            server_config.clone(),
        );
        extractor2.setup_pool_download(Some(pool_id_to_test), true); // last_first = true

        let (post_sender2, mut post_receiver2) = unbounded_channel();
        let limit2 = 1u16;

        let fetch_handle2 = extractor2.setup_fetch_thread(post_sender2, None, Some(limit2), None);

        let mut received_posts2 = Vec::new();
        while let Some(post) = post_receiver2.recv().await {
            received_posts2.push(post);
        }
        join!(fetch_handle2).0.unwrap().unwrap();

        assert_eq!(
            received_posts2.len(),
            limit2 as usize,
            "Test Case 2: Incorrect number of posts received"
        );
        for post in received_posts2 {
            assert_eq!(
                post.website,
                ImageBoards::E621,
                "Test Case 2: Post from incorrect website"
            );
        }

        // Test Case 3: No limit, last_first = false (fetch all)
        let mut extractor3 = PostExtractor::new(
            &Vec::<String>::new(),
            &[],
            true,
            false,
            E621Api::new(),
            server_config,
        );
        extractor3.setup_pool_download(Some(pool_id_to_test), false);

        let (post_sender3, mut post_receiver3) = unbounded_channel();
        let fetch_handle3 = extractor3.setup_fetch_thread(post_sender3, None, None, None); // No limit
        let mut received_posts3 = Vec::new();
        while let Some(post) = post_receiver3.recv().await {
            received_posts3.push(post);
        }
        join!(fetch_handle3).0.unwrap().unwrap();
        assert_eq!(
            received_posts3.len(),
            all_expected_post_ids_in_order.len(),
            "Test Case 3: Should fetch all posts"
        );
        for post in received_posts3 {
            assert_eq!(
                post.website,
                ImageBoards::E621,
                "Test Case 3: Post website mismatch"
            );
        }
    }
}
