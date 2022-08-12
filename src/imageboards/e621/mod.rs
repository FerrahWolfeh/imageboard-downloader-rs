use std::path::PathBuf;
use anyhow::Error;

mod models;

const E621_POST_LIST: &str = "https://e621.net/posts.json";
const E621_FAVORITES: &str = "https://e621.net/favorites.json";

pub struct E621Downloader {
    tag_string: String,
    tag_list: Vec<String>
}

impl E621Downloader {
    pub fn new(
        tags: &[String],
        out_dir: Option<PathBuf>,
        concurrent_downs: usize,
        safe_mode: bool,
    ) -> Result<Self, Error> {


        Ok(Self {
            tag_string: "".to_string(),
            tag_list: Vec::from(tags)
        })
    }
}
