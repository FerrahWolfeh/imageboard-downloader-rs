use std::sync::Arc;

use ibdl_common::tokio::task::{JoinHandle, spawn_local};
use ibdl_common::{all_ratings, post::rating::Rating};
use ibdl_extractors::error::ExtractorError;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

// Assuming your Post struct is in ibdl_common::post::Post
// and it can be made Serialize + Deserialize. Need to ensure Post is indeed serializable.
// The WebPost struct suggests it might not be directly serializable, hence the conversion.

// Need tokio imports for channels and tasks, adapted for WASM.
// Use the re-exported ones from ibdl_common
use ibdl_common::post::Post;
use ibdl_common::tokio::sync::{
    Mutex,
    mpsc::{UnboundedReceiver, UnboundedSender},
}; // Assuming you have an enum like this

use ibdl_extractors::prelude::*;

// Your extractors - you'll need to figure out how to instantiate and use them.
// This is a simplified example. The actual extractor API from your project will be used.
use ibdl_extractors::imageboards::{
    // ... other extractors
    danbooru::DanbooruExtractor,
    e621::E621Extractor,
};

// Helper for logging to the browser console
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

// A simplified struct to pass to JavaScript.
// Your `Post` struct might be too complex or contain non-serializable types.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebPost {
    pub id: u64,
    pub direct_url: String, // URL to the image/file itself
    pub post_url: String,   // URL to the imageboard post page
    pub tags: Vec<String>,
    pub site: String,
    // Add other fields you want to display like thumbnail_url, rating, etc.
}

// Conversion from your main `Post` struct to `WebPost`
// This is an example, you'll need to adapt it based on your `Post` struct fields
impl WebPost {
    fn from_common_post(common_post: &Post, imageboard_name: &str) -> Self {
        let domain = common_post.website.domain();
        WebPost {
            id: common_post.id,
            direct_url: common_post.url.clone(),
            post_url: format!("https://{}/posts/{}", domain, common_post.id),
            tags: common_post
                .tags
                .iter()
                .map(|tag| tag.tag().to_string())
                .collect(),
            site: imageboard_name.to_string(),
        }
    }
}

#[wasm_bindgen(start)]
pub fn main_js() -> Result<(), JsValue> {
    // Optional: set up a panic hook for better debugging in the browser console
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
    console_log!("WASM module initialized.");
    Ok(())
}

#[wasm_bindgen]
pub async fn fetch_links(
    site_name: String,
    tags_str: String,
    limit: Option<u16>,
) -> Result<JsValue, JsValue> {
    console_log!(
        "Rust (WASM): fetch_links called with site '{}', tags '{}', limit {:?}",
        site_name,
        tags_str,
        limit
    );

    let tags: Vec<String> = tags_str.split_whitespace().map(String::from).collect();
    if tags.is_empty() {
        return Err(JsValue::from_str("No tags provided."));
    }

    // Use the channel-based async fetch methods from unsync.rs

    // Create an unbounded channel to receive posts
    let (sender, mut receiver): (UnboundedSender<Post>, UnboundedReceiver<Post>) =
        ibdl_common::tokio::sync::mpsc::unbounded_channel();

    // Use Arc<Mutex> to share the collected posts vector between tasks
    let shared_posts = Arc::new(Mutex::new(Vec::new()));
    let receiver_posts = shared_posts.clone();

    // Spawn a task to receive posts from the channel and collect them
    let receiver_handle = spawn_local(async move {
        console_log!("Rust (WASM): Receiver task started.");
        let mut posts = receiver_posts.lock().await;
        while let Some(post) = receiver.recv().await {
            posts.push(post);
        }
        console_log!("Rust (WASM): Receiver task finished.");
    });

    // Spawn the extractor task using setup_fetch_thread
    let extractor_handle: JoinHandle<Result<u64, ExtractorError>> =
        match site_name.to_lowercase().as_str() {
            "danbooru" => {
                // Example: Instantiate and use DanbooruExtractor
                // The exact method signature and parameters will depend on your actual extractor implementation.
                // The extractor's search/fetch method MUST be async and use a WASM-compatible reqwest client.
                let extractor = DanbooruExtractor::new(&tags, all_ratings!(), true, false); // Adapt constructor
                console_log!("Rust (WASM): Using DanbooruExtractor");
                // Use setup_fetch_thread to spawn the async_fetch logic
                extractor.setup_fetch_thread(sender, Some(1), limit, None) // Start page 1, pass limit, no post_counter
            }
            "e621" => {
                let extractor = E621Extractor::new(&tags, all_ratings!(), true, false); // Adapt constructor
                console_log!("Rust (WASM): Using E621Extractor");
                // Use setup_fetch_thread to spawn the async_fetch logic
                extractor.setup_fetch_thread(sender, Some(1), limit, None) // Start page 1, pass limit, no post_counter
            }
            // Add other sites your `ibdl-extractors` support
            _ => {
                // If site is unsupported, drop the sender immediately to close the channel
                drop(sender);
                // Return an error immediately
                return Err(JsValue::from_str(&format!(
                    "Unsupported site: {}",
                    site_name
                )));
            }
        };

    // Wait for the extractor task to finish. This will also cause the sender to be dropped.
    let extractor_result = extractor_handle.await;

    // Wait for the receiver task to finish processing all messages
    let _ = receiver_handle.await;

    // Process the result from the extractor task
    match extractor_result {
        Ok(total_removed) => {
            console_log!(
                "Rust (WASM): Extractor finished. {:?} posts removed by blacklist.",
                total_removed
            );
            // Access the collected posts from the shared vector
            let final_posts = shared_posts.lock().await;
            console_log!("Rust (WASM): Collected {} posts.", final_posts.len());

            if final_posts.is_empty() {
                return Err(JsValue::from_str("No posts found for the given tags."));
            }

            let web_posts: Vec<WebPost> = final_posts
                .iter()
                .map(|p| WebPost::from_common_post(p, &site_name))
                .collect();

            Ok(serde_wasm_bindgen::to_value(&web_posts)?) // Serialize to JsValue
        }
        Err(e) => {
            console_log!("Rust (WASM): Extractor error: {}", e);
            Err(JsValue::from_str(&format!("Extractor error: {}", e)))
        }
    }
}
