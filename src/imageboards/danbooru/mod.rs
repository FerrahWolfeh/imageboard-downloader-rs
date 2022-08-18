use crate::imageboards::auth::ImageboardConfig;
use crate::imageboards::common::{generate_out_dir, try_auth, Post, ProgressArcs};
use crate::imageboards::ImageBoards;
use crate::progress_bars::master_progress_style;
use crate::{client, join_tags};
use anyhow::{bail, Error};
use colored::Colorize;
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget};
use log::{debug, trace};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::fs::create_dir_all;
use tokio::time::Instant;

pub struct DanbooruDownloader {
    item_count: u64,
    page_count: f32,
    concurrent_downloads: usize,
    tag_list: Vec<String>,
    tag_string: String,
    client: Client,
    out_dir: PathBuf,
    safe_mode: bool,
    save_as_id: bool,
    downloaded_files: Arc<Mutex<u64>>,
    blacklisted_posts: u64,
}

impl DanbooruDownloader {
    pub async fn new(
        tags: &[String],
        out_dir: Option<PathBuf>,
        concurrent_downs: usize,
        safe_mode: bool,
        auth_state: bool,
        save_as_id: bool,
    ) -> Result<Self, Error> {
        if tags.len() > 2 {
            bail!("Danbooru downloader currently doesn't support more than 2 tags")
        };
        // Use common client for all connections with a set User-Agent
        let client = client!(ImageBoards::Danbooru.user_agent());

        // Join tags to a url format in case there's more than one
        let tag_string = join_tags!(tags);

        // Place downloaded items in current dir or in /tmp
        let out = generate_out_dir(out_dir, &tag_string, ImageBoards::Danbooru)?;

        // Try to authenticate, does nothing if auth flag is not set
        try_auth(auth_state, ImageBoards::Danbooru, &client).await?;

        Ok(Self {
            item_count: 0,
            page_count: 0.0,
            concurrent_downloads: concurrent_downs,
            tag_list: Vec::from(tags),
            tag_string,
            client,
            out_dir: out,
            safe_mode,
            save_as_id,
            downloaded_files: Arc::new(Mutex::new(0)),
            blacklisted_posts: 0,
        })
    }

    async fn get_post_count(&mut self, auth_creds: &Option<ImageboardConfig>) -> Result<(), Error> {
        let count_endpoint = format!(
            "{}?tags={}",
            ImageBoards::Danbooru
                .post_count_url(self.safe_mode)
                .unwrap(),
            &self.tag_string
        );

        // Get an estimate of total posts and pages to search
        let count = if let Some(data) = auth_creds {
            debug!("[AUTH] Fetching post count");
            self.client
                .get(count_endpoint)
                .basic_auth(&data.username, Some(&data.api_key))
                .send()
                .await?
                .json::<Value>()
                .await?
        } else {
            debug!("Fetching post count");
            self.client
                .get(count_endpoint)
                .send()
                .await?
                .json::<Value>()
                .await?
        };

        if let Some(count) = count["counts"]["posts"].as_u64() {
            // Bail out if no posts are found
            if count == 0 {
                bail!("No posts found for tag selection!")
            }

            self.item_count = count;
            self.page_count = (self.item_count as f32 / 200.0).ceil();

            debug!(
                "{} Posts for tag list '{:?}'",
                &self.item_count, &self.tag_list
            );
        } else {
            bail!("Danbooru returned a malformed JSON response while fetching post count.")
        }
        Ok(())
    }

    pub async fn download(&mut self) -> Result<(), Error> {
        // Get auth data
        let auth_res = ImageBoards::Danbooru.read_config_from_fs().await?;

        // Generate post count data
        Self::get_post_count(self, &auth_res).await?;

        // Create output dir
        create_dir_all(&self.out_dir).await?;

        // Setup global progress bar
        let bar = ProgressBar::new(self.item_count).with_style(master_progress_style(
            &ImageBoards::Danbooru.progress_template(),
        ));
        bar.set_draw_target(ProgressDrawTarget::stderr_with_hz(60));
        bar.enable_steady_tick(Duration::from_millis(100));

        // Initialize the bar
        let multi = Arc::new(MultiProgress::new());
        let main = Arc::new(multi.add(bar));

        let bars = Arc::new(ProgressArcs { main, multi });

        // Begin downloading all posts per page
        for i in 1..=self.page_count as u64 {
            // Check safe mode
            let url_mode = format!(
                "{}?tags={}",
                ImageBoards::Danbooru.post_url(self.safe_mode).unwrap(),
                &self.tag_string
            );

            // Fetch item list from page
            let jj = if let Some(data) = &auth_res {
                debug!("[AUTH] Fetching posts from page {}", i);
                self.client
                    .get(url_mode)
                    .query(&[("page", &i.to_string()), ("limit", &200.to_string())])
                    .basic_auth(&data.username, Some(&data.api_key))
                    .send()
                    .await?
                    .json::<Value>()
                    .await?
            } else {
                debug!("Fetching posts from page {}", i);
                self.client
                    .get(url_mode)
                    .query(&[("page", &i.to_string()), ("limit", &200.to_string())])
                    .send()
                    .await?
                    .json::<Value>()
                    .await?
            };

            let start_point = Instant::now();
            let mut posts: Vec<Post> = jj
                .as_array()
                .unwrap()
                .iter()
                .filter(|c| c["file_url"].as_str().is_some())
                .map(|c| {
                    let mut tag_list = HashSet::new();

                    for i in c["tag_string"].as_str().unwrap().split(' ') {
                        if !i.contains("//") {
                            tag_list.insert(i.to_string());
                        }
                    }

                    Post {
                        id: c["id"].as_u64().unwrap(),
                        md5: c["md5"].as_str().unwrap().to_string(),
                        url: c["file_url"].as_str().unwrap().to_string(),
                        extension: c["file_ext"].as_str().unwrap().to_string(),
                        tags: tag_list,
                    }
                })
                .collect();

            if let Some(auth) = &auth_res {
                posts.retain(|c| {
                    c.tags
                        .iter()
                        .any(|s| !auth.user_data.blacklisted_tags.contains(s))
                });
                self.blacklisted_posts += 1;
            }
            let end_iter = Instant::now();
            trace!("Post mapping took {:?}", end_iter - start_point);

            // Download everything got in the above function
            futures::stream::iter(posts)
                .map(|d| Self::download_item(self, d, bars.clone()))
                .buffer_unordered(self.concurrent_downloads)
                .collect::<Vec<Result<(), Error>>>()
                .await;
        }

        bars.main.finish_and_clear();
        println!(
            "{} {} {}",
            self.downloaded_files
                .lock()
                .unwrap()
                .to_string()
                .bold()
                .green(),
            "files".bold().green(),
            "downloaded".bold()
        );
        if auth_res.is_some() && self.blacklisted_posts > 0 {
            println!(
                "{} {}",
                self.blacklisted_posts.to_string().bold().red(),
                "posts with blacklisted tags were not downloaded."
                    .bold()
                    .red()
            )
        }
        Ok(())
    }

    async fn download_item(&self, item: Post, bars: Arc<ProgressArcs>) -> Result<(), Error> {
        item.get(
            &self.client,
            &self.out_dir,
            bars,
            ImageBoards::Danbooru,
            self.downloaded_files.clone(),
            self.save_as_id,
        )
        .await?;
        Ok(())
    }
}
