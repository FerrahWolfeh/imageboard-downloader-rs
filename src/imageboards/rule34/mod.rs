use crate::extract_ext_from_url;
use crate::imageboards::common::{generate_out_dir, Post, ProgressArcs};
use crate::imageboards::ImageBoards;
use crate::progress_bars::master_progress_style;
use crate::{client, join_tags};
use anyhow::{bail, Error};
use colored::Colorize;
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget};
use log::debug;
use reqwest::Client;
use roxmltree::Document;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::fs::create_dir_all;

pub struct R34Downloader {
    item_count: usize,
    page_count: usize,
    client: Client,
    tag_string: String,
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
            page_count: 0,
            client,
            tag_string,
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
            .text()
            .await?;

        let num = Document::parse(count)
            .unwrap()
            .root_element()
            .attribute("count")
            .unwrap()
            .parse::<usize>()
            .unwrap();

        // Bail out if no posts are found
        if num == 0 {
            bail!("No posts found for tag selection!")
        }
        debug!("Tag list is valid");

        // Fill memory with standard post count just to initialize the progress bar
        self.item_count = num;
        self.page_count = (self.item_count as f32 / 1000.0).ceil() as usize;

        self.posts_endpoint = count_endpoint;

        Ok(())
    }

    pub async fn download(&mut self) -> Result<(), Error> {
        // Generate post count data
        Self::check_tag_list(self).await?;

        // Create output dir
        create_dir_all(&self.out_dir).await?;

        // Setup global progress bar
        let bar = ProgressBar::new(self.item_count as u64).with_style(master_progress_style(
            &ImageBoards::Rule34.progress_template(),
        ));
        bar.set_draw_target(ProgressDrawTarget::stderr_with_hz(60));
        bar.enable_steady_tick(Duration::from_millis(100));

        // Initialize the bars
        let multi = Arc::new(MultiProgress::new());
        let main = Arc::new(multi.add(bar));

        let bars = Arc::new(ProgressArcs { main, multi });

        // Begin downloading all posts per page
        for i in 0..=self.page_count {
            let items = &self
                .client
                .get(&self.posts_endpoint)
                .query(&[("pid", i), ("limit", 1000)])
                .send()
                .await?
                .text()
                .await?;

            let doc = Document::parse(items)?;
            let stuff: Vec<Post> = doc
                .root_element()
                .children()
                .filter(|c| c.attribute("file_url").is_some())
                .map(|c| {
                    let file = c.attribute("file_url").unwrap();
                    Post {
                        id: c.attribute("id").unwrap().parse::<u64>().unwrap(),
                        url: file.to_string(),
                        md5: c.attribute("md5").unwrap().to_string(),
                        extension: extract_ext_from_url!(file),
                        tags: Default::default(),
                    }
                })
                .collect();

            futures::stream::iter(stuff)
                .map(|d| Self::download_item(self, d, bars.clone()))
                .buffer_unordered(self.concurrent_downloads)
                .collect::<Vec<_>>()
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
                .blue(),
            "files".bold().blue(),
            "downloaded".bold()
        );
        Ok(())
    }

    async fn download_item(&self, item: Post, bars: Arc<ProgressArcs>) -> Result<(), Error> {
        item.get(
            &self.client,
            &self.out_dir,
            bars,
            ImageBoards::Realbooru,
            self.downloaded_files.clone(),
            self.save_as_id,
        )
            .await?;
        Ok(())
    }
}
