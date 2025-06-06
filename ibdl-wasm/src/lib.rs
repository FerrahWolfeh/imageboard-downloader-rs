use ibdl_common::post::Post;
use ibdl_extractors::blacklist::{DEFAULT_BLACKLIST_TOML, GlobalBlacklist};
use ibdl_extractors::error::ExtractorError;
use ibdl_extractors::extractor::PostExtractor; // Added ExtractorError
use ibdl_extractors::extractor_config::DEFAULT_SERVERS; // Added ServerConfig for clarity, though PostExtractor::new uses it directly
use ibdl_extractors::imageboards::prelude::*;
use ibdl_extractors::prelude::AsyncFetch;
use serde::Serialize;
use tokio::sync::mpsc;
use wasm_bindgen::prelude::*; // Use tokio's MPSC channel

// Helper to log to the browser console
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}
macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

// The WebPost DTO and its From<Post> implementation remain the same,
// as they are still a great way to prepare data for JS.
#[derive(Serialize, Debug, Clone)]
pub struct WebPost {
    pub id: u64,
    pub direct_url: String,
    pub post_url: String,
    pub tags: Vec<String>,
    pub site: String,
    pub rating: String,
}

impl From<&Post> for WebPost {
    fn from(post: &Post) -> Self {
        let domain = post.website.domain();
        Self {
            id: post.id,
            direct_url: post.url.clone(),
            post_url: format!("https://{}/posts/{}", domain, post.id),
            tags: post.tags.iter().map(|t| t.tag().to_string()).collect(),
            site: format!("{:?}", post.website),
            rating: format!("{:?}", post.rating),
        }
    }
}

// Enum to handle different concrete PostExtractor types
enum AnyPostExtractor {
    Danbooru(PostExtractor<DanbooruApi>),
    E621(PostExtractor<E621Api>),
    // If you support more sites, add their variants here
}

impl AnyPostExtractor {
    // This method's signature is simplified to the parameters actually used in this context.
    // It will call the underlying PostExtractor's async_fetch method with the
    // appropriate arguments (including `None` for blacklisted_tags and cbz_mode).
    async fn async_fetch(
        &mut self,
        sender: mpsc::UnboundedSender<Post>,
        limit: Option<u16>,
    ) -> Result<u64, ExtractorError> {
        match self {
            AnyPostExtractor::Danbooru(extractor) => {
                // Call the original async_fetch, passing None for unused args
                extractor.async_fetch(sender, None, limit, None).await
            }
            AnyPostExtractor::E621(extractor) => {
                // Call the original async_fetch, passing None for unused args
                extractor.async_fetch(sender, None, limit, None).await
            } // Add cases for other sites if AnyPostExtractor is expanded
        }
    }
}

#[wasm_bindgen]
pub async fn fetch_links(
    site_name: String,
    tags_str: String,
    limit: u16,
) -> Result<JsValue, JsValue> {
    console_log!(
        "[WASM] Fetching from '{}' with tags '{}'",
        site_name,
        tags_str
    );

    let tags: Vec<String> = tags_str.split_whitespace().map(String::from).collect();
    if tags.is_empty() {
        return Err("No tags provided.".into());
    }
    let tags_slice: Vec<&str> = tags.iter().map(AsRef::as_ref).collect();

    // This part is a bit tricky, the API structs need the server config
    let server_cfg = DEFAULT_SERVERS
        .get(&site_name)
        .ok_or_else(|| JsValue::from_str(&format!("Invalid site name: {}", site_name)))?
        .clone();

    let global_blacklist = GlobalBlacklist::from_config(DEFAULT_BLACKLIST_TOML).unwrap();

    // Create the extractor instance using the enum. It must be mutable.
    let mut extractor: AnyPostExtractor = match site_name.to_lowercase().as_str() {
        "danbooru" => AnyPostExtractor::Danbooru(PostExtractor::new(
            &tags_slice,
            &global_blacklist,
            &[],
            true,
            false,
            DanbooruApi::new(),
            server_cfg,
        )),
        "e621" => AnyPostExtractor::E621(PostExtractor::new(
            &tags_slice,
            &global_blacklist,
            &[],
            true,
            false,
            E621Api::new(),
            server_cfg,
        )),
        _ => {
            return Err(JsValue::from_str(&format!(
                "Unsupported site: {}",
                site_name
            )));
        }
    };
    // 1. Create the channel
    let (sender, mut receiver) = mpsc::unbounded_channel::<Post>();

    // 2. Define the collector future
    let collector_future = async {
        let mut collected_posts = Vec::new();
        while let Some(post) = receiver.recv().await {
            collected_posts.push(post);
        }
        console_log!(
            "[WASM] Collector finished, got {} posts.",
            collected_posts.len()
        );
        collected_posts
    };

    // 3. Define the fetcher future
    // Call the async_fetch method defined on our AnyPostExtractor enum.
    let fetcher_future = extractor.async_fetch(sender, Some(limit));

    // 4. Run futures. In WASM, true concurrency via `join!` might not be ideal or available.
    // We can run the fetcher and then collect.
    // If the fetcher needs to run "in the background" while the collector waits,
    // wasm-bindgen-futures::spawn_local would be used for the fetcher.
    // However, since the collector depends on the sender from the fetcher,
    // and the sender is dropped when fetcher_future completes,
    // we can await the fetcher first, then the collector will naturally complete.

    console_log!("[WASM] Starting fetcher and collector...");

    // Await the fetcher. This will run until the sender is dropped.
    let fetch_result = fetcher_future.await;

    // Await the collector. Since the sender is now dropped, receiver.recv() will return None,
    // and the collector_future will complete.
    let collected_posts = collector_future.await;

    // 5. Handle the results
    if let Err(e) = fetch_result {
        let error_message = format!("Error during fetch: {:?}", e);
        console_log!("[WASM] {}", error_message);
        return Err(error_message.into());
    }

    console_log!("[WASM] Fetch successful. Converting posts to WebPosts.");
    let web_posts: Vec<WebPost> = collected_posts.iter().map(WebPost::from).collect();

    // Serialize to JsValue and return
    serde_wasm_bindgen::to_value(&web_posts).map_err(|e| e.into())
}

#[wasm_bindgen]
pub async fn fetch_links_proxy(
    site_name: String,
    tags_str: String,
    limit: u16,
) -> Result<JsValue, JsValue> {
    // The original URL to the imageboard API
    let original_api_url = format!(
        "https://{}/posts.json?limit={}&tags={}",
        // This part is a bit simplified, you'd need the correct domain from your configs
        if site_name == "danbooru" {
            "danbooru.donmai.us"
        } else {
            "e621.net"
        },
        limit,
        tags_str
    );

    // Prepend the public CORS proxy URL
    // The proxy needs the full original URL passed to it.
    let proxy_url = format!(
        "https://corsproxy.io/?{}",
        urlencoding::encode(&original_api_url)
    );

    console_log!("[WASM] Querying via public proxy: {}", proxy_url);

    // The rest of the function remains the same!
    let client = reqwest::Client::new();
    let res = client
        .get(&proxy_url)
        .send()
        .await
        .map_err(|e| JsValue::from_str(&format!("Network Error: {}", e)))?;

    // ... same error handling and JSON parsing ...
    if !res.status().is_success() {
        let error_body = res
            .text()
            .await
            .unwrap_or_else(|_| "Failed to get error body".to_string());
        return Err(JsValue::from_str(&format!("Proxy Error: {}", error_body)));
    }

    // Now, you need to deserialize the JSON from the API response
    // THIS WILL LIKELY FAIL if you deserialize to `Vec<Post>` because the JSON structure
    // from Danbooru/e621 is not a direct Vec<Post>. You need a temporary struct.
    #[derive(serde::Deserialize)]
    struct ApiPost {
        // Define fields you need, e.g., id, file_url, tag_string, etc.
        id: u64,
        file_url: Option<String>,
        source: Option<String>,
        large_file_url: Option<String>,
        tag_string: String,
        rating: String,
    }

    let api_posts: Vec<ApiPost> = res
        .json()
        .await
        .map_err(|e| JsValue::from_str(&format!("JSON Deserialization Error: {}", e)))?;

    // Convert from the API structure to your WebPost structure
    let web_posts: Vec<WebPost> = api_posts
        .iter()
        .map(|p| {
            let direct_url = p
                .large_file_url
                .clone()
                .or(p.file_url.clone())
                .unwrap_or_default();
            WebPost {
                id: p.id,
                direct_url: direct_url.clone(),
                post_url: format!(
                    "https://{}/posts/{}",
                    if site_name == "danbooru" {
                        "danbooru.donmai.us"
                    } else {
                        "e621.net"
                    },
                    p.id
                ),
                tags: p.tag_string.split_whitespace().map(String::from).collect(),
                site: site_name.clone(),
                rating: p.rating.clone(),
            }
        })
        .collect();

    serde_wasm_bindgen::to_value(&web_posts).map_err(|e| e.into())
}
