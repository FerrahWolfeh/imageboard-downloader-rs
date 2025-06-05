//! Post extractor for `https://konachan.com` and other Moebooru imageboards

use ibdl_common::post::extension::Extension;
use ibdl_common::post::tags::{Tag, TagType};
use ibdl_common::{
    extract_ext_from_url, join_tags,
    log::debug,
    post::{rating::Rating, Post},
    serde_json, ImageBoards,
};
use std::time::Duration;

use crate::extractor::caps::ExtractorFeatures;
use crate::extractor::SiteApi;
use crate::{error::ExtractorError, imageboards::moebooru::models::KonachanPost};

mod models;

// Define the MoebooruApi struct
pub struct MoebooruApi;

impl MoebooruApi {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for MoebooruApi {
    fn default() -> Self {
        Self::new()
    }
}

// Implement the SiteApi trait for MoebooruApi
impl SiteApi for MoebooruApi {
    type PostListResponse = Vec<KonachanPost>;
    type SinglePostResponse = (); // Moebooru standard API doesn't support single post fetch
    type PoolDetailsResponse = (); // Moebooru standard API doesn't support pools

    fn deserialize_post_list(&self, data: &str) -> Result<Self::PostListResponse, ExtractorError> {
        let response = serde_json::from_str::<Vec<KonachanPost>>(data)?;
        Ok(response)
    }

    fn deserialize_single_post(
        &self,
        _data: &str,
    ) -> Result<Self::SinglePostResponse, ExtractorError> {
        Err(ExtractorError::UnsupportedOperation)
    }

    fn map_post_list_response(
        &self,
        response: Self::PostListResponse,
    ) -> Result<Vec<Post>, ExtractorError> {
        let post_iter = response.into_iter().filter(|c| c.file_url.is_some());

        let mapper_iter = post_iter.map(|c| {
            let url = c.file_url.clone().unwrap(); // Filtered out None already

            let tag_iter = c.tags.split(' ');

            let mut tags = Vec::with_capacity(tag_iter.size_hint().0);

            let ext = extract_ext_from_url!(url);

            tag_iter.for_each(|i| {
                tags.push(Tag::new(i, TagType::Any));
            });

            Post {
                id: c.id.unwrap_or(0), // Default to 0 if ID is missing (shouldn't happen)
                website: ImageBoards::Moebooru,
                url,
                md5: c.md5.unwrap_or_else(|| "unknown".to_string()), // Default if MD5 is missing
                extension: Extension::guess_format(&ext),
                tags,
                rating: Rating::from_rating_str(&c.rating),
            }
        });

        Ok(mapper_iter.collect::<Vec<Post>>())
    }

    fn map_single_post_response(
        &self,
        _response: Self::SinglePostResponse,
    ) -> Result<Post, ExtractorError> {
        Err(ExtractorError::UnsupportedOperation)
    }

    fn single_post_url(&self, _base_url: &str, _post_id: u32) -> String {
        // This should ideally not be called if features() is correct, but provide a placeholder
        debug!("Attempted to call single_post_url on MoebooruApi, which does not support single post fetch.");
        String::new() // Return empty string, the caller should handle the UnsupportedOperation error
    }

    fn posts_url(
        &self,
        base_url: &str, // Expected to be like "https://konachan.com/post.json"
        page: u16,
        limit: u16,
        tags_query_string: &str,
    ) -> String {
        // Moebooru uses 'page' and 'limit' query parameters.
        // base_url is expected to be like "https://konachan.com/post.json"
        // So, additional parameters are appended with '&' or '?' if it's the first.
        // Assuming base_url might already have '?', we'll always use '&'.
        let mut url = format!("{base_url}&page={page}&limit={limit}");

        if !tags_query_string.is_empty() {
            url.push_str("&tags=");
            url.push_str(tags_query_string);
        }
        url
    }

    fn process_tags(&mut self, input_tags: &[String]) -> (String, Vec<String>) {
        // Moebooru uses space to separate tags in the query string
        let tag_string = join_tags!(input_tags);
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
        // Add a small delay to be polite to the server
        Some(Duration::from_millis(500))
    }

    fn multi_get_post_api_call_delay(&self) -> Duration {
        // This method is not used as single post fetch is unsupported, but provide a value
        Duration::from_millis(500)
    }

    fn imageboard_type(&self) -> ImageBoards {
        ImageBoards::Moebooru
    }

    fn features() -> ExtractorFeatures {
        // Moebooru supports TagSearch and AsyncFetch.
        // It does NOT support PoolExtract, Auth, or SinglePostFetch.
        ExtractorFeatures::from_bits_truncate(
            ExtractorFeatures::AsyncFetch.bits() | ExtractorFeatures::TagSearch.bits(),
        )
    }

    // Pool methods (unsupported for standard Moebooru API)
    fn pool_details_url(&self, _base_url: &str, _pool_id: u32) -> String {
        debug!("Attempted to call pool_details_url on MoebooruApi, which does not support pools.");
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
    ) -> Result<ahash::HashMap<u64, usize>, ExtractorError> {
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

    use crate::{
        extractor::PostExtractor, extractor_config::DEFAULT_SERVERS,
        imageboards::prelude::MoebooruApi,
    };

    #[tokio::test]
    async fn post_api() {
        let server_config = DEFAULT_SERVERS.get("konachan").unwrap().clone();

        let api = MoebooruApi::new();

        let extractor = PostExtractor::new(
            &["1girl", "long_hair"],
            &[],
            false,
            false,
            api, // Pass the MoebooruApi instance
            server_config,
        );

        let post_list = extractor.get_post_list(1, Some(10)).await;
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
        assert_eq!(first_post.website, ImageBoards::Moebooru);
        assert!(!first_post.md5.is_empty());
        assert!(!first_post.url.is_empty());
        assert_ne!(first_post.rating, Rating::Unknown);
        assert!(first_post.tags.iter().any(|tag| tag.tag() == "1girl"));
        assert!(first_post.tags.iter().any(|tag| tag.tag() == "long_hair"));
    }
}
