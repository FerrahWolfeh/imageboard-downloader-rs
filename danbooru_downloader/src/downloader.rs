use crate::{DanbooruItem, DanbooruPostCount};
use anyhow::{bail, Error};
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget};
use log::debug;

use reqwest::Client;
use std::path::{PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::{create_dir_all, OpenOptions};
use tokio::io::AsyncWriteExt;

use crate::progress_bar::{download_progress_style, master_progress_style};

const DANBOORU_COUNT: &str = "https://danbooru.donmai.us/counts/posts.json?tags=";

#[derive(Debug)]
pub struct Downloader {
    item_list: Vec<DanbooruItem>,
    item_count: u64,
    page_count: u64,
    concurrent_downloads: usize,
    tag_string: String,
    client: Client,
    out_dir: PathBuf,
}

impl Downloader {
    pub async fn new(
        tags: &[String],
        out_dir: Option<String>,
        concurrent_downs: usize,
    ) -> Result<Self, Error> {
        // Use common client for all connections with a set User-Agent (mostly because of e621)
        let client = Client::builder()
            .user_agent("LibSFA 0.5 - testing")
            .build()?;

        // Place downloaded items in current dir or in /tmp
        let place = match out_dir {
            None => std::env::current_dir()?,
            Some(dir) => PathBuf::from(dir),
        };

        // Join tags to a url format in case there's more than one
        let tag_url = tags.join("+");
        debug!("Tag List: {}", tag_url);

        // Create output dir
        let out = place.join(PathBuf::from(&tag_url));
        create_dir_all(&out).await?;
        debug!("Target dir: {}", out.display());

        // Get an estimate of total posts and pages to search
        let count = client
            .get(format!("{}{}", DANBOORU_COUNT, tag_url))
            .send()
            .await?
            .json::<DanbooruPostCount>()
            .await?;

        let total = (count.counts.posts / 200.0).ceil() as u64;

        Ok(Self {
            item_list: vec![],
            item_count: count.counts.posts as u64,
            page_count: total,
            concurrent_downloads: concurrent_downs,
            tag_string: tag_url,
            client,
            out_dir: out,
        })
    }

    pub async fn download(&mut self) -> Result<(), Error> {
        // Setup global progress bar
        let bar = ProgressBar::new(self.item_count).with_style(master_progress_style());
        bar.set_draw_target(ProgressDrawTarget::stderr_with_hz(60));
        bar.enable_steady_tick(Duration::from_millis(100));

        // Initialize the bar
        let multi = Arc::new(MultiProgress::new());
        let main = Arc::new(multi.add(bar));

        // Begin downloading all posts per page
        for i in 1..=self.page_count {
            // Fetch item list from page
            let jj = self
                .client
                .get(format!(
                    "https://danbooru.donmai.us/posts.json?tags={}&page={}&limit=200",
                    self.tag_string, i
                ))
                .send()
                .await?
                .json::<Vec<DanbooruItem>>()
                .await?;

            // Download everything got in the above function
            let downs = futures::stream::iter(jj)
                .map(|d| Self::fetch(self, d, multi.clone(), main.clone()))
                .buffer_unordered(self.concurrent_downloads)
                .collect::<Vec<_>>()
                .await;

            for i in downs {
                self.item_list.extend(i?);
            }
        }
        main.finish_and_clear();
        Ok(())
    }

    async fn fetch(
        &self,
        item: DanbooruItem,
        multi: Arc<MultiProgress>,
        main: Arc<ProgressBar>,
    ) -> Result<Vec<DanbooruItem>, Error> {
        let mut storage = Vec::new();

        if item.file_url.is_some() {
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

            let output = &self.out_dir.join(format!(
                "{}.{}",
                item.md5.clone().unwrap(),
                item.file_ext.clone().unwrap()
            ));

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

            storage.push(item);
        } else {
        }
        Ok(storage)
    }
}
