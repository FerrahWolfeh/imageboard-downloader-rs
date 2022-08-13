use crate::imageboards::common::generate_out_dir;
use crate::imageboards::e621::models::{E621File, E621Post, E621TopLevel};
use crate::imageboards::E621_UA;
use crate::progress_bars::{download_progress_style, master_progress_style};
use crate::{client, join_tags, ImageBoards};
use anyhow::{bail, Error};
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget};
use log::debug;
use md5::compute;
use reqwest::Client;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::fs::{create_dir_all, read, OpenOptions};
use tokio::io::AsyncWriteExt;

mod models;

const E621_POST_LIST: &str = "https://e621.net/posts.json";
const E926_POST_LIST: &str = "https://e926.net/posts.json";
const E621_FAVORITES: &str = "https://e621.net/favorites.json";

pub struct E621Downloader {
    item_count: u64,
    client: Client,
    tag_string: String,
    tag_list: Vec<String>,
    concurrent_downloads: usize,
    count_endpoint: String,
    out_dir: PathBuf,
    safe_mode: bool,
}

impl E621Downloader {
    pub fn new(
        tags: &[String],
        out_dir: Option<PathBuf>,
        concurrent_downs: usize,
        safe_mode: bool,
    ) -> Result<Self, Error> {
        // Use common client for all connections with a set User-Agent (e621 requires this)
        let client = client!(E621_UA);

        // Join tags to a url format in case there's more than one
        let tag_string = join_tags!(tags);

        // Place downloaded items in current dir or in /tmp
        let out = generate_out_dir(out_dir, &tag_string, ImageBoards::E621)?;

        Ok(Self {
            item_count: 0,
            client,
            tag_string,
            tag_list: Vec::from(tags),
            concurrent_downloads: concurrent_downs,
            count_endpoint: "".to_string(),
            out_dir: out,
            safe_mode,
        })
    }

    async fn check_tag_list(&mut self) -> Result<(), Error> {
        let count_endpoint = if self.safe_mode {
            format!("{}?tags={}", E926_POST_LIST, &self.tag_string)
        } else {
            format!("{}?tags={}", E621_POST_LIST, &self.tag_string)
        };

        // Get an estimate of total posts and pages to search
        let count = &self
            .client
            .get(&count_endpoint)
            .send()
            .await?
            .json::<E621TopLevel>()
            .await?;

        // Bail out if no posts are found
        if count.posts.is_empty() {
            bail!("No posts found for tag selection!")
        } else {
            debug!("Tag list: {:?} is valid", &self.tag_list);

            // Fill memory with standard post count just to initialize the progress bar
            self.item_count = count.posts.len() as u64;

            self.count_endpoint = count_endpoint;

            Ok(())
        }
    }

    async fn check_file_exists(
        &self,
        item: &E621Post,
        multi_progress: Arc<MultiProgress>,
        main_bar: Arc<ProgressBar>,
    ) -> Result<(), Error> {
        if item.file.url.is_some() {
            let post_file = &item.file;
            let output = &self.out_dir.join(format!(
                "{}.{}",
                post_file.md5.as_ref().unwrap(),
                post_file.ext.as_ref().unwrap()
            ));
            if output.exists() {
                let file_digest = compute(read(output).await?);
                let hash = format!("{:x}", file_digest);
                if &hash != post_file.md5.as_ref().unwrap() {
                    fs::remove_file(output).await?;
                    multi_progress.println(format!(
                        "File {}.{} is corrupted. Re-downloading...",
                        post_file.md5.as_ref().unwrap(),
                        post_file.ext.as_ref().unwrap()
                    ))?;
                    Self::fetch(self, item, multi_progress, main_bar, output).await?
                } else {
                    multi_progress.println(format!(
                        "File {}.{} already exists. Skipping.",
                        post_file.md5.as_ref().unwrap(),
                        post_file.ext.as_ref().unwrap()
                    ))?;
                    main_bar.inc(1)
                }
                return Ok(());
            } else {
                Self::fetch(self, item, multi_progress, main_bar, output).await?
            }
        }
        Ok(())
    }

    pub async fn download(&mut self) -> Result<(), Error> {
        // Generate post count data
        Self::check_tag_list(self).await?;

        // Create output dir
        create_dir_all(&self.out_dir).await?;

        // Setup global progress bar
        let bar = ProgressBar::new(0).with_style(master_progress_style());
        bar.set_draw_target(ProgressDrawTarget::stderr_with_hz(60));
        bar.enable_steady_tick(Duration::from_millis(100));

        // Initialize the bar
        let multi = Arc::new(MultiProgress::new());
        let main = Arc::new(multi.add(bar));

        // Keep track of pages already downloaded
        let mut page = 0;

        // Begin downloading all posts per page
        // Since e621 doesn't give a precise number of posts, we'll need to go through the pages until there's no more posts left to show
        while self.item_count != 0 {
            page += 1;

            let items = &self
                .client
                .get(&self.count_endpoint)
                .query(&[("page", page), ("limit", 320)])
                .send()
                .await?
                .json::<E621TopLevel>()
                .await?;

            let posts = items.posts.len() as u64;

            self.item_count = posts;
            main.inc_length(posts);

            if self.item_count != 0 {
                futures::stream::iter(&items.posts)
                    .map(|d| Self::check_file_exists(self, d, multi.clone(), main.clone()))
                    .buffer_unordered(self.concurrent_downloads)
                    .collect::<Vec<_>>()
                    .await;
            }
        }

        main.finish_and_clear();
        Ok(())
    }

    async fn fetch(
        &self,
        item: &E621Post,
        multi: Arc<MultiProgress>,
        main: Arc<ProgressBar>,
        output: &Path,
    ) -> Result<(), Error> {
        debug!("Fetching {}", &item.file.url.as_ref().unwrap());
        let res = self
            .client
            .get(item.file.url.as_ref().unwrap())
            .send()
            .await?;

        let size = res.content_length().unwrap_or_default();
        let bar = ProgressBar::new(size).with_style(download_progress_style());
        bar.set_draw_target(ProgressDrawTarget::stderr_with_hz(60));

        let pb = multi.add(bar);

        debug!("Creating destination file {:?}", &output);
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(output)
            .await?;

        // Download the file chunk by chunk.
        debug!("Retrieving chunks...");
        let mut stream = res.bytes_stream();
        while let Some(item) = stream.next().await {
            // Retrieve chunk.
            let mut chunk = match item {
                Ok(chunk) => chunk,
                Err(e) => {
                    bail!(e)
                }
            };
            pb.inc(chunk.len() as u64);

            // Write to file.
            match file.write_all_buf(&mut chunk).await {
                Ok(_res) => (),
                Err(e) => {
                    bail!(e);
                }
            };
        }
        pb.finish_and_clear();

        main.inc(1);
        Ok(())
    }
}
