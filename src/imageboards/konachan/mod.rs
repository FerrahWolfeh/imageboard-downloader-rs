use crate::imageboards::common::{generate_out_dir, Post, ProgressArcs};
use crate::imageboards::konachan::models::KonachanPost;
use crate::imageboards::ImageBoards;
use crate::progress_bars::master_progress_style;
use crate::{
    client, extract_ext_from_url, finish_and_print_results, initialize_progress_bars, join_tags,
};
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

mod models;

pub struct KonachanDownloader {
    item_count: usize,
    client: Client,
    tag_string: String,
    tag_list: Vec<String>,
    concurrent_downloads: usize,
    posts_endpoint: String,
    out_dir: PathBuf,
    save_as_id: bool,
    safe_mode: bool,
    _download_limit: Option<usize>,
    downloaded_files: Arc<Mutex<u64>>,
}

impl KonachanDownloader {
    pub fn new(
        tags: &[String],
        out_dir: Option<PathBuf>,
        concurrent_downs: usize,
        download_limit: Option<usize>,
        safe_mode: bool,
        save_as_id: bool,
    ) -> Result<Self, Error> {
        // Use common client for all connections with a set User-Agent
        let client = client!(ImageBoards::Konachan.user_agent());

        // Join tags to a url format in case there's more than one
        let tag_string = join_tags!(tags);

        // Place downloaded items in current dir or in /tmp
        let out = generate_out_dir(out_dir, &tag_string, ImageBoards::Konachan)?;

        Ok(Self {
            item_count: 0,
            client,
            tag_string,
            tag_list: Vec::from(tags),
            concurrent_downloads: concurrent_downs,
            posts_endpoint: "".to_string(),
            out_dir: out,
            save_as_id,
            safe_mode,
            _download_limit: download_limit,
            downloaded_files: Arc::new(Mutex::new(0)),
        })
    }

    async fn check_tag_list(&mut self) -> Result<(), Error> {
        let count_endpoint = format!(
            "{}?tags={}",
            ImageBoards::Konachan.post_url(self.safe_mode).unwrap(),
            &self.tag_string
        );

        // Get an estimate of total posts and pages to search
        let count = &self
            .client
            .get(&count_endpoint)
            .send()
            .await?
            .json::<Vec<KonachanPost>>()
            .await?;

        // Bail out if no posts are found
        if count.is_empty() {
            bail!("No posts found for tag selection!")
        }
        debug!("Tag list: {:?} is valid", &self.tag_list);

        // Fill memory with standard post count just to initialize the progress bar
        self.item_count = count.len();

        self.posts_endpoint = count_endpoint;

        Ok(())
    }

    pub async fn download(&mut self) -> Result<(), Error> {
        // Generate post count data
        Self::check_tag_list(self).await?;

        // Create output dir
        create_dir_all(&self.out_dir).await?;

        // Setup global progress bar
        let bars = initialize_progress_bars!(0, ImageBoards::Konachan);

        // Keep track of pages already downloaded
        let mut page = 1;

        // Begin downloading all posts per page
        // Since konachan doesn't give a precise number of posts, we'll need to go through the pages until there's no more posts left to show
        while self.item_count != 0 {
            let items = &self
                .client
                .get(&self.posts_endpoint)
                .query(&[("page", page), ("limit", 100)])
                .send()
                .await?
                .json::<Vec<KonachanPost>>()
                .await?;

            let posts = items.len();

            self.item_count = posts;

            bars.main.inc_length(posts as u64);

            if self.item_count != 0 {
                futures::stream::iter(items)
                    .map(|d| Self::download_item(self, d, bars.clone()))
                    .buffer_unordered(self.concurrent_downloads)
                    .collect::<Vec<_>>()
                    .await;
            }
            page += 1;
        }

        finish_and_print_results!(bars, self);

        Ok(())
    }

    async fn download_item(
        &self,
        item: &KonachanPost,
        bars: Arc<ProgressArcs>,
    ) -> Result<(), Error> {
        if item.file_url.is_some() {
            let extension = extract_ext_from_url!(item.file_url.as_ref().unwrap());
            let entity = Post {
                id: item.id.unwrap(),
                url: item.file_url.clone().unwrap(),
                md5: item.md5.clone().unwrap(),
                extension,
                tags: Default::default(),
            };
            entity
                .get(
                    &self.client,
                    &self.out_dir,
                    bars,
                    ImageBoards::Konachan,
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
