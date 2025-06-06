use ibdl_common::post::Post;
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

    // Create the extractor instance using the enum. It must be mutable.
    let mut extractor: AnyPostExtractor = match site_name.to_lowercase().as_str() {
        "danbooru" => AnyPostExtractor::Danbooru(PostExtractor::new(
            &tags_slice,
            &[],
            true,
            false,
            DanbooruApi::new(),
            server_cfg,
        )),
        "e621" => AnyPostExtractor::E621(PostExtractor::new(
            &tags_slice,
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

    // 4. Run both futures concurrently
    console_log!("[WASM] Starting fetcher and collector...");
    let (fetch_result, collected_posts) = tokio::join!(fetcher_future, collector_future);

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
