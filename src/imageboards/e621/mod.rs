use crate::imageboards::common::{generate_out_dir, try_auth, Post, ProgressArcs};
use crate::imageboards::e621::models::{E621Post, E621TopLevel};
use crate::progress_bars::master_progress_style;
use crate::{client, join_tags, ImageBoards, ImageboardConfig};
use anyhow::{bail, Error};
use colored::Colorize;
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget};
use log::debug;
use reqwest::Client;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::fs::create_dir_all;

pub mod models;

const _E621_FAVORITES: &str = "https://e621.net/favorites.json";

pub struct E621Downloader {
    item_count: u64,
    client: Client,
    tag_string: String,
    tag_list: Vec<String>,
    concurrent_downloads: usize,
    posts_endpoint: String,
    out_dir: PathBuf,
    safe_mode: bool,
    save_as_id: bool,
    downloaded_files: Arc<Mutex<u64>>,
    _blacklisted_posts: u64,
}

impl E621Downloader {
    pub async fn new(
        tags: &[String],
        out_dir: Option<PathBuf>,
        concurrent_downs: usize,
        auth_state: bool,
        safe_mode: bool,
        save_as_id: bool,
    ) -> Result<Self, Error> {
        // Use common client for all connections with a set User-Agent (e621 requires this)
        let client = client!(ImageBoards::E621.user_agent());

        // Join tags to a url format in case there's more than one
        let tag_string = join_tags!(tags);

        // Place downloaded items in current dir or in /tmp
        let out = generate_out_dir(out_dir, &tag_string, ImageBoards::E621)?;

        // Try to authenticate, does nothing if auth flag is not set
        try_auth(auth_state, ImageBoards::E621, &client).await?;

        Ok(Self {
            item_count: 0,
            client,
            tag_string,
            tag_list: Vec::from(tags),
            concurrent_downloads: concurrent_downs,
            posts_endpoint: "".to_string(),
            out_dir: out,
            safe_mode,
            save_as_id,
            downloaded_files: Arc::new(Mutex::new(0)),
            _blacklisted_posts: 0,
        })
    }

    async fn check_tag_list(&mut self, auth_creds: &Option<ImageboardConfig>) -> Result<(), Error> {
        let count_endpoint = format!(
            "{}?tags={}",
            ImageBoards::E621.post_url(self.safe_mode).unwrap(),
            &self.tag_string
        );

        // Get an estimate of total posts and pages to search
        let count = if let Some(data) = auth_creds {
            debug!("[AUTH] Checking tags");
            self.client
                .get(&count_endpoint)
                .basic_auth(&data.username, Some(&data.api_key))
                .send()
                .await?
                .json::<E621TopLevel>()
                .await?
        } else {
            debug!("Checking tags");
            self.client
                .get(&count_endpoint)
                .send()
                .await?
                .json::<E621TopLevel>()
                .await?
        };

        // Bail out if no posts are found
        if count.posts.is_empty() {
            bail!("No posts found for tag selection!")
        }
        debug!("Tag list: {:?} is valid", &self.tag_list);

        // Fill memory with standard post count just to initialize the progress bar
        self.item_count = count.posts.len() as u64;

        self.posts_endpoint = count_endpoint;

        Ok(())
    }

    pub async fn download(&mut self) -> Result<(), Error> {
        // Get auth data
        let auth_res = ImageBoards::E621.read_config_from_fs().await?;

        // Generate post count data
        Self::check_tag_list(self, &auth_res).await?;

        // Create output dir
        create_dir_all(&self.out_dir).await?;

        // Setup global progress bar
        let bar = ProgressBar::new(0).with_style(master_progress_style(
            &ImageBoards::E621.progress_template(),
        ));
        bar.set_draw_target(ProgressDrawTarget::stderr_with_hz(60));
        bar.enable_steady_tick(Duration::from_millis(100));

        // Initialize the bars
        let multi = Arc::new(MultiProgress::new());
        let main = Arc::new(multi.add(bar));

        let bars = Arc::new(ProgressArcs { main, multi });

        // Keep track of pages already downloaded
        let mut page = 0;

        // Begin downloading all posts per page
        // Since e621 doesn't give a precise number of posts, we'll need to go through the pages until there's no more posts left to show
        while self.item_count != 0 {
            page += 1;

            let items = if let Some(data) = &auth_res {
                debug!("[AUTH] Fetching posts from page {}", page);
                self.client
                    .get(&self.posts_endpoint)
                    .query(&[("page", page), ("limit", 320)])
                    .basic_auth(&data.username, Some(&data.api_key))
                    .send()
                    .await?
                    .json::<E621TopLevel>()
                    .await?
            } else {
                debug!("Fetching posts from page {}", page);
                self.client
                    .get(&self.posts_endpoint)
                    .query(&[("page", page), ("limit", 320)])
                    .send()
                    .await?
                    .json::<E621TopLevel>()
                    .await?
            };

            let posts = items.posts.len() as u64;

            self.item_count = posts;

            bars.main.inc_length(posts);

            if self.item_count != 0 {
                futures::stream::iter(&items.posts)
                    .map(|d| Self::download_item(self, d, bars.clone()))
                    .buffer_unordered(self.concurrent_downloads)
                    .collect::<Vec<_>>()
                    .await;
            }
        }

        bars.main.finish_and_clear();
        println!(
            "{} {} {}",
            self.downloaded_files
                .lock()
                .unwrap()
                .to_string()
                .bold()
                .blue(),
            "files".bold().blue(),
            "downloaded".bold()
        );
        Ok(())
    }

    async fn download_item(&self, item: &E621Post, bars: Arc<ProgressArcs>) -> Result<(), Error> {
        if item.file.url.is_some() {
            let entity = Post {
                id: item.id.unwrap(),
                url: item.file.url.clone().unwrap(),
                md5: item.file.md5.clone().unwrap(),
                extension: item.file.ext.clone().unwrap(),
                tags: Default::default(),
            };
            entity
                .get(
                    &self.client,
                    &self.out_dir,
                    bars,
                    ImageBoards::E621,
                    self.downloaded_files.clone(),
                    self.save_as_id,
                )
                .await?;
            Ok(())
        } else {
            bars.main.set_length(bars.main.length().unwrap() - 1);
            Ok(())
        }
    }
}
