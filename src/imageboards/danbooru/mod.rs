use crate::imageboards::common::generate_out_dir;
use crate::imageboards::danbooru::models::{DanbooruItem, DanbooruPostCount};
use crate::imageboards::DANBOORU_UA;
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

const SAFEBOORU_COUNT: &str = "https://safebooru.donmai.us/counts/posts.json";
const SAFEBOORU_POSTS: &str = "https://safebooru.donmai.us/posts.json";
const DANBOORU_COUNT: &str = "https://danbooru.donmai.us/counts/posts.json";
const DANBOORU_POSTS: &str = "https://danbooru.donmai.us/posts.json";

pub struct DanbooruDownloader {
    item_count: u64,
    page_count: u64,
    concurrent_downloads: usize,
    tag_list: Vec<String>,
    tag_string: String,
    client: Client,
    out_dir: PathBuf,
    safe_mode: bool,
    _downloaded_files: u64,
}

impl DanbooruDownloader {
    pub fn new(
        tags: &[String],
        out_dir: Option<PathBuf>,
        concurrent_downs: usize,
        safe_mode: bool,
    ) -> Result<Self, Error> {
        if tags.len() > 2 {
            bail!("Danbooru downloader currently doesn't support more than 2 tags")
        };
        // Use common client for all connections with a set User-Agent
        let client = client!(DANBOORU_UA);

        // Join tags to a url format in case there's more than one
        let tag_string = join_tags!(tags);

        // Place downloaded items in current dir or in /tmp
        let out = generate_out_dir(out_dir, &tag_string, ImageBoards::Danbooru)?;

        Ok(Self {
            item_count: 0,
            page_count: 0,
            concurrent_downloads: concurrent_downs,
            tag_list: Vec::from(tags),
            tag_string,
            client,
            out_dir: out,
            safe_mode,
            _downloaded_files: 0,
        })
    }

    async fn get_post_count(&mut self) -> Result<(), Error> {
        let count_endpoint = if self.safe_mode {
            SAFEBOORU_COUNT
        } else {
            DANBOORU_COUNT
        };

        // Get an estimate of total posts and pages to search
        let count = &self
            .client
            .get(count_endpoint)
            .query(&[("tags", &self.tag_string)])
            .send()
            .await?
            .json::<DanbooruPostCount>()
            .await?;

        // Bail out if no posts are found
        if count.counts.posts == 0.0 {
            bail!("No posts found for tag selection!")
        }

        self.item_count = count.counts.posts as u64;
        self.page_count = (count.counts.posts / 200.0).ceil() as u64;

        debug!(
            "{} Posts for tag list '{:?}'",
            &self.item_count, &self.tag_list
        );

        Ok(())
    }

    pub async fn download(&mut self) -> Result<(), Error> {
        // Generate post count data
        Self::get_post_count(self).await?;

        // Create output dir
        create_dir_all(&self.out_dir).await?;

        // Setup global progress bar
        let bar = ProgressBar::new(self.item_count).with_style(master_progress_style());
        bar.set_draw_target(ProgressDrawTarget::stderr_with_hz(60));
        bar.enable_steady_tick(Duration::from_millis(100));

        // Initialize the bar
        let multi = Arc::new(MultiProgress::new());
        let main = Arc::new(multi.add(bar));

        // Begin downloading all posts per page
        for i in 1..=self.page_count {
            // Check safe mode
            let url_mode = if self.safe_mode {
                SAFEBOORU_POSTS
            } else {
                DANBOORU_POSTS
            };

            // Fetch item list from page
            let jj = self
                .client
                .get(url_mode)
                .query(&[
                    ("tags", &self.tag_string),
                    ("page", &i.to_string()),
                    ("limit", &200.to_string()),
                ])
                .send()
                .await?
                .json::<Vec<DanbooruItem>>()
                .await?;

            // Download everything got in the above function
            futures::stream::iter(jj)
                .map(|d| Self::check_file_exists(self, d, multi.clone(), main.clone()))
                .buffer_unordered(self.concurrent_downloads)
                .collect::<Vec<_>>()
                .await;
        }
        main.finish_and_clear();
        Ok(())
    }

    async fn check_file_exists(
        &self,
        item: DanbooruItem,
        multi_progress: Arc<MultiProgress>,
        main_bar: Arc<ProgressBar>,
    ) -> Result<(), Error> {
        if item.file_url.is_some() {
            let output = &self.out_dir.join(format!(
                "{}.{}",
                item.md5.clone().unwrap(),
                item.file_ext.clone().unwrap()
            ));
            if output.exists() {
                let file_digest = compute(read(output).await?);
                let hash = format!("{:x}", file_digest);
                if hash != item.md5.clone().unwrap() {
                    fs::remove_file(output).await?;
                    multi_progress.println(format!(
                        "File {}.{} is corrupted. Re-downloading...",
                        item.md5.clone().unwrap(),
                        item.file_ext.clone().unwrap()
                    ))?;
                    Self::fetch(self, &item, multi_progress, main_bar, output).await?
                } else {
                    multi_progress.println(format!(
                        "File {}.{} already exists. Skipping.",
                        item.md5.unwrap(),
                        item.file_ext.unwrap()
                    ))?;
                    main_bar.set_length(main_bar.length().unwrap() - 1)
                }
                return Ok(());
            } else {
                Self::fetch(self, &item, multi_progress, main_bar, output).await?
            }
        }
        Ok(())
    }

    async fn fetch(
        &self,
        item: &DanbooruItem,
        multi: Arc<MultiProgress>,
        main: Arc<ProgressBar>,
        output: &Path,
    ) -> Result<(), Error> {
        debug!("Fetching {}", &item.file_url.clone().unwrap());
        let res = self
            .client
            .get(item.file_url.clone().unwrap())
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
