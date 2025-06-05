//! Post extractor for `https://danbooru.donmai.us`
//!
//! The danbooru extractor has the following features:
//! - Authentication
//! - Native blacklist (defined in user profile page)
//!
use self::models::DanbooruPost;
use crate::error::ExtractorError;
use crate::extractor::caps::ExtractorFeatures;
use crate::extractor::SiteApi;
use crate::imageboards::danbooru::models::DanbooruPoolList; // Use PostExtractor from generic
use ahash::HashMap;
use ibdl_common::post::extension::Extension;
use ibdl_common::serde_json;
use ibdl_common::{
    join_tags,
    log::debug,
    post::{rating::Rating, Post},
    ImageBoards,
};
use std::time::Duration;

mod models;

/// API logic for Danbooru.
#[derive(Debug, Clone, Default)]
pub struct DanbooruApi {
    // DanbooruApi might have its own fields for configuration or state if needed.
    // For now, it's a marker struct that implements SiteApi.
}

impl DanbooruApi {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl SiteApi for DanbooruApi {
    type PostListResponse = Vec<DanbooruPost>;
    type SinglePostResponse = DanbooruPost;

    fn deserialize_post_list(&self, data: &str) -> Result<Self::PostListResponse, ExtractorError> {
        let post_list = serde_json::from_str(data)?;
        Ok(post_list)
    }

    fn deserialize_single_post(
        &self,
        data: &str,
    ) -> Result<Self::SinglePostResponse, ExtractorError> {
        let post = serde_json::from_str(data)?;
        Ok(post)
    }

    // This is the equivalent of the old DanbooruExtractor::map_posts
    fn map_post_list_response(
        &self,
        posts: Self::PostListResponse,
    ) -> Result<Vec<Post>, ExtractorError> {
        let batch = posts.into_iter().filter(|c| c.file_url.is_some());

        let mapper_iter = batch.map(|c| {
            let tag_list = c.map_tags();

            let rt = c.rating.as_deref().unwrap_or("g"); // Default to general if rating is missing
            let rating = if rt == "s" {
                Rating::Questionable
            } else {
                Rating::from_rating_str(rt)
            };

            Post {
                id: c.id.unwrap_or_default(),
                website: ImageBoards::Danbooru,
                md5: c.md5.unwrap_or_default(),
                url: c.file_url.unwrap_or_default(), // Filtered earlier, should be present
                extension: Extension::guess_format(&c.file_ext.unwrap_or_default()),
                tags: tag_list,
                rating,
            }
        });

        Ok(mapper_iter.collect())
    }

    // This is the equivalent of the old DanbooruExtractor::map_post
    fn map_single_post_response(
        &self,
        post_model: Self::SinglePostResponse,
    ) -> Result<Post, ExtractorError> {
        let tag_list = post_model.map_tags();

        let rt = post_model.rating.as_deref().unwrap_or("g");
        let rating = if rt == "s" {
            Rating::Questionable
        } else {
            Rating::from_rating_str(rt)
        };

        Ok(Post {
            id: post_model.id.ok_or(ExtractorError::MissingField {
                field: "id".to_string(),
            })?,
            website: ImageBoards::Danbooru,
            md5: post_model.md5.ok_or(ExtractorError::MissingField {
                field: "md5".to_string(),
            })?,
            url: post_model.file_url.ok_or(ExtractorError::MissingField {
                field: "url".to_string(),
            })?,
            extension: Extension::guess_format(&post_model.file_ext.ok_or(
                ExtractorError::MissingField {
                    field: "extension".to_string(),
                },
            )?),
            tags: tag_list,
            rating,
        })
    }

    fn single_post_url(&self, base_url: &str, post_id: u32) -> String {
        format!("{base_url}/{post_id}.json")
    }

    fn posts_url(
        &self,
        base_url: &str,
        page_num: u16,
        limit: u16,
        tags_query_string: &str,
    ) -> String {
        // Danbooru uses 'page' (1-indexed) and 'limit'.
        // base_url is typically "https://danbooru.donmai.us/posts.json"
        // It doesn't have query parameters yet, so start with '?'
        let mut url = format!("{base_url}?limit={limit}&page={page_num}");

        if !tags_query_string.is_empty() {
            url.push_str("&tags=");
            url.push_str(tags_query_string);
        }
        url
    }

    fn process_tags(&mut self, input_tags: &[String]) -> (String, Vec<String>) {
        // Danbooru API allows a maximum of 2 tags in a search query.
        // The first two tags are used for the API query.
        // All input_tags are returned as tags_for_post_queue for client-side filtering
        // and for the PostQueue itself.
        let api_query_tags: Vec<String> = input_tags.iter().take(2).cloned().collect();
        let tags_for_query = join_tags!(api_query_tags);
        let tags_for_post_queue = input_tags.to_vec();

        debug!("DanbooruApi processed_tags for API query: '{tags_for_query}'");
        debug!("DanbooruApi tags for PostQueue (and positive filtering): {tags_for_post_queue:?}");

        (tags_for_query, tags_for_post_queue)
    }

    fn full_search_page_limit(&self) -> u16 {
        100 // Danbooru's typical practical limit for deep searches without specific pagination cursors
    }

    fn full_search_post_limit_break_condition(&self, posts_fetched_this_page: usize) -> bool {
        // Danbooru returns fewer than requested if it's the last page or near rate limits.
        // A common strategy is to stop if a page returns 0 posts.
        // PostExtractor's full_search loop already handles breaking on 0 posts.
        // This condition can be used if the API guarantees to return less than `limit`
        // only on the very last page. For Danbooru, 0 posts is a clearer signal.
        posts_fetched_this_page == 0
    }

    fn full_search_api_call_delay(&self) -> Option<Duration> {
        Some(Duration::from_millis(500)) // Danbooru is sensitive to rapid requests
    }

    fn multi_get_post_api_call_delay(&self) -> Duration {
        Duration::from_millis(500)
    }

    fn imageboard_type(&self) -> ImageBoards {
        ImageBoards::Danbooru
    }

    fn features() -> ExtractorFeatures {
        // This is what DanbooruExtractor::features() used to return.
        // Note: Extractor::features() is static, so DanbooruExtractor will implement it directly.
        // This method on SiteApi is for PostExtractor to query features if Extractor::features() were non-static.
        ExtractorFeatures::from_bits_truncate(0b0001_1111) // AsyncFetch + TagSearch + SinglePostDownload + PoolDownload + Auth
    }

    type PoolDetailsResponse = DanbooruPoolList;

    fn pool_details_url(&self, base_url: &str, pool_id: u32) -> String {
        // `base_url` is expected to be like "https://danbooru.donmai.us/pools"
        // from ServerConfig.pool_idx_url
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
        // The DanbooruPoolList directly contains post_ids in order.
        // We need to map them to (post_id, 0-based_index).
        let map = response
            .post_ids
            .into_iter()
            .enumerate()
            .map(|(index, post_id)| (post_id, index))
            .collect();
        Ok(map)
    }

    fn parse_post_ids_from_pool_json_str(
        &self,
        raw_json: &str,
    ) -> Result<Vec<u64>, ExtractorError> {
        // This method is for when you only need the Vec<u64> of post_ids.
        let pool_list: DanbooruPoolList =
            serde_json::from_str(raw_json).map_err(ExtractorError::JsonSerializeFail)?;
        Ok(pool_list.post_ids)
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
        imageboards::danbooru::DanbooruApi,
        prelude::{AsyncFetch, PoolExtract},
    };

    #[tokio::test]
    async fn post_api() {
        let server_config = DEFAULT_SERVERS.get("danbooru").unwrap().clone();

        let danbooru_api = DanbooruApi::new();

        let extractor = PostExtractor::new(
            &["1girl", "cyrene_(honkai:_star_rail)"],
            &[],
            false,
            false,
            danbooru_api, // Pass the DanbooruApi instance
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
        assert_eq!(first_post.website, ImageBoards::Danbooru);
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

        let extractor = PostExtractor::new(
            tags_to_search,
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

    #[tokio::test]
    async fn pool_download_fetch_idxs() {
        // 1. Setup: Get ServerConfig for Danbooru
        // Ensure DEFAULT_SERVERS.danbooru.pool_idx_url is correctly set,
        // e.g., "https://danbooru.donmai.us/pools"
        let server_config = DEFAULT_SERVERS
            .get("danbooru")
            .expect("Danbooru server config not found")
            .clone();

        assert!(
            server_config.pool_idx_url.is_some(),
            "Danbooru server config is missing 'pool_idx_url'"
        );

        // 2. Create DanbooruApi
        let danbooru_api = DanbooruApi::new();

        // 3. Create PostExtractor
        // Tags, ratings, etc., are not strictly needed for fetch_pool_idxs itself,
        // but PostExtractor requires them for initialization.
        let mut extractor = PostExtractor::new(
            &Vec::<String>::new(), // No tags needed for this specific pool fetch
            &[],                   // No ratings needed
            true,                  // disable_blacklist
            false,                 // map_videos
            danbooru_api,
            server_config,
        );

        // 4. Define pool ID and expected posts for pool 18845
        let pool_id_to_test = 25903; // Updated pool ID
        let expected_post_ids_in_order: Vec<u64> = vec![
            9_388_833, 9_394_551, 9_396_748, 9_399_848, 9_400_219, 9_404_553, 9_410_251, 9_410_674,
            9_415_510,
        ];

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
        let limit = 5u16;
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
            .get("danbooru")
            .expect("Danbooru server config not found")
            .clone();

        assert!(
            server_config.pool_idx_url.is_some(),
            "Danbooru server config is missing 'pool_idx_url'"
        );

        let pool_id_to_test = 25903;
        let all_expected_post_ids_in_order: Vec<u64> = vec![
            9_388_833, 9_394_551, 9_396_748, 9_399_848, 9_400_219, 9_404_553, 9_410_251, 9_410_674,
            9_415_510,
        ];

        // Test Case 1: Limit 3, last_first = false
        let mut extractor1 = PostExtractor::new(
            &Vec::<String>::new(), // No tags for pool download
            &[],                   // No specific ratings
            true,                  // disable_blacklist
            false,                 // map_videos
            DanbooruApi::new(),
            server_config.clone(),
        );
        extractor1.setup_pool_download(Some(pool_id_to_test), false); // last_first = false

        let (post_sender1, mut post_receiver1) = unbounded_channel();
        let limit1 = 3u16;

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
                ImageBoards::Danbooru,
                "Test Case 1: Post from incorrect website"
            );
        }

        // Test Case 2: Limit 2, last_first = true
        let mut extractor2 = PostExtractor::new(
            &Vec::<String>::new(),
            &[],
            true,
            false,
            DanbooruApi::new(),
            server_config.clone(),
        );
        extractor2.setup_pool_download(Some(pool_id_to_test), true); // last_first = true

        let (post_sender2, mut post_receiver2) = unbounded_channel();
        let limit2 = 2u16;

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
                ImageBoards::Danbooru,
                "Test Case 2: Post from incorrect website"
            );
        }

        // Test Case 3: No limit, last_first = false (fetch all)
        let mut extractor3 = PostExtractor::new(
            &Vec::<String>::new(),
            &[],
            true,
            false,
            DanbooruApi::new(),
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
                ImageBoards::Danbooru,
                "Test Case 3: Post website mismatch"
            );
        }
    }
}
