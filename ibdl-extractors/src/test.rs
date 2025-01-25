#![cfg(test)]
use crate::imageboards::danbooru::DanbooruExtractor;
use crate::imageboards::Extractor;
use ibdl_common::{
    post::{rating::Rating},
    tokio, ImageBoards,
};
use crate::extractor_config::DEFAULT_SERVERS;

#[tokio::test]
async fn danbooru_test_post_api() {
    let server_config = DEFAULT_SERVERS.get("danbooru").unwrap().clone();

    let extractor = DanbooruExtractor::new_with_config(&["1girl"], &[], false, false, server_config);

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
async fn e621_test_post_api() {
    let server_config = DEFAULT_SERVERS.get("e621").unwrap().clone();

    let extractor = DanbooruExtractor::new_with_config(&["solo"], &[], false, false, server_config);

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
    assert!(first_post.tags.iter().any(|tag| tag.tag() == "solo"));
}

#[tokio::test]
async fn gelbooru_test_post_api() {
    let server_config = DEFAULT_SERVERS.get("gelbooru").unwrap().clone();

    let extractor = DanbooruExtractor::new_with_config(&["1girl"], &[], false, false, server_config);

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
async fn gelbooru_v0_2_test_post_api() {
    let server_config = DEFAULT_SERVERS.get("realbooru").unwrap().clone();

    let extractor = DanbooruExtractor::new_with_config(&["1girl"], &[], false, false, server_config);

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
async fn moebooru_test_post_api() {
    let server_config = DEFAULT_SERVERS.get("konachan").unwrap().clone();

    let extractor = DanbooruExtractor::new_with_config(&["1girl"], &[], false, false, server_config);

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