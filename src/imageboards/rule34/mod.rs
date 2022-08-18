mod models;

use crate::imageboards::common::{generate_out_dir, Post, ProgressArcs};
use crate::imageboards::rule34::models::R34Post;
use crate::progress_bars::master_progress_style;
use crate::{client, join_tags};
use crate::{extract_ext_from_url, ImageBoards};
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

pub struct R34Downloader {
    item_count: usize,
    client: Client,
    tag_string: String,
    tag_list: Vec<String>,
    concurrent_downloads: usize,
    posts_endpoint: String,
    out_dir: PathBuf,
    save_as_id: bool,
    downloaded_files: Arc<Mutex<u64>>,
}

impl R34Downloader {
    pub fn new(
        tags: &[String],
        out_dir: Option<PathBuf>,
        concurrent_downs: usize,
        save_as_id: bool,
    ) -> Result<Self, Error> {
        // Use common client for all connections with a set User-Agent
        let client = client!(ImageBoards::Rule34.user_agent());

        // Join tags to a url format in case there's more than one
        let tag_string = join_tags!(tags);

        // Place downloaded items in current dir or in /tmp
        let out = generate_out_dir(out_dir, &tag_string, ImageBoards::Rule34)?;

        Ok(Self {
            item_count: 0,
            client,
            tag_string,
            tag_list: Vec::from(tags),
            concurrent_downloads: concurrent_downs,
            posts_endpoint: "".to_string(),
            out_dir: out,
            save_as_id,
            downloaded_files: Arc::new(Mutex::new(0)),
        })
    }

    async fn check_tag_list(&mut self) -> Result<(), Error> {
        let count_endpoint = format!(
            "{}&tags={}",
            ImageBoards::Rule34.post_url(false).unwrap(),
            &self.tag_string
        );

        // Get an estimate of total posts and pages to search
        let count = &self
            .client
            .get(&count_endpoint)
            .send()
            .await?
            .json::<Vec<R34Post>>()
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
        let bar = ProgressBar::new(0).with_style(master_progress_style(
            &ImageBoards::Rule34.progress_template(),
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
        // Since rule34 doesn't give a precise number of posts, we'll need to go through the pages until there's no more posts left to show
        while self.item_count != 0 {
            let items = &self
                .client
                .get(&self.posts_endpoint)
                .query(&[("pid", page), ("limit", 1000)])
                .send()
                .await?
                .json::<Vec<R34Post>>()
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

    async fn download_item(&self, item: &R34Post, bars: Arc<ProgressArcs>) -> Result<(), Error> {
        if item.file_url.is_some() {
            let extension = extract_ext_from_url!(item.image.as_ref().unwrap());
            let entity = Post {
                id: item.id.unwrap(),
                url: item.file_url.clone().unwrap(),
                md5: item.hash.clone().unwrap(),
                extension,
                tags: Default::default(),
            };
            entity
                .get(
                    &self.client,
                    &self.out_dir,
                    bars,
                    ImageBoards::Rule34,
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
