//! Download logic for Gelbooru-based imageboards
//!
//! The gelbooru downloader has the following features:
//! * Multiple simultaneous downloads.
//!
//! ***
//!
//! This downloader is compatible with these imageboards:
//! * `Imageboards::Rule34`
//! * `Imageboards::Realbooru`
//! * `Imageboards::Gelbooru`
//!
//! # Example usage
//!
//! ```rust
//! use std::path::PathBuf;
//! use imageboard_downloader::{ImageBoards, GelbooruDownloader};
//!
//! // Input tags
//! let tags = vec!["umbreon".to_string(), "espeon".to_string()];
//!
//! // Dir where all will be saved
//! let output = Some(PathBuf::from("./"));
//!
//! // Number of simultaneous downloads
//! let sd = 3;
//!
//! // Disable download of NSFW posts
//! let safe_mode = true;
//!
//! // Login to the imageboard (only needs to be true once)
//! let auth = true;
//!
//! // Save files with as <post_id>.png rather than <image_md5>.png
//! let save_as_id = false;
//!
//! // Limit number of downloaded files
//! let limit = Some(100);
//!
//! // Initialize the downloader
//! // In this case, download from Rule34
//! let mut dl = GelbooruDownloader::new(ImageBoards::Rule34, &tags, output, sd, limit, save_as_id)?;
//!
//! // Download
//! dl.download().await?;
//! ```
use super::queue::DownloadQueue;
use crate::imageboards::common::{generate_out_dir, Counters};
use crate::imageboards::post::Post;
use crate::imageboards::ImageBoards;
use crate::progress_bars::ProgressArcs;
use crate::{client, join_tags};
use crate::{extract_ext_from_url, finish_and_print_results};
use anyhow::{bail, Error};
use colored::Colorize;
use log::debug;
use reqwest::Client;
use roxmltree::Document;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::fs::create_dir_all;

pub struct GelbooruDownloader {
    active_imageboard: ImageBoards,
    item_count: usize,
    page_count: usize,
    client: Client,
    tag_string: String,
    concurrent_downloads: usize,
    posts_endpoint: String,
    out_dir: PathBuf,
    save_as_id: bool,
    download_limit: Option<usize>,
    counters: Counters,
}

impl GelbooruDownloader {
    pub fn new(
        imageboard: ImageBoards,
        tags: &[String],
        out_dir: Option<PathBuf>,
        concurrent_downs: usize,
        download_limit: Option<usize>,
        save_as_id: bool,
    ) -> Result<Self, Error> {
        // Use common client for all connections with a set User-Agent
        let client = client!(imageboard.user_agent());

        // Join tags to a url format in case there's more than one
        let tag_string = join_tags!(tags);

        // Place downloaded items in current dir or in /tmp
        let out = generate_out_dir(out_dir, &tags, imageboard)?;

        Ok(Self {
            active_imageboard: imageboard,
            item_count: 0,
            page_count: 0,
            client,
            tag_string,
            concurrent_downloads: concurrent_downs,
            posts_endpoint: "".to_string(),
            out_dir: out,
            save_as_id,
            download_limit,
            counters: Counters {
                total_mtx: Arc::new(Mutex::new(0)),
                downloaded_mtx: Arc::new(Mutex::new(0)),
            },
        })
    }

    async fn get_post_count(&mut self) -> Result<(), Error> {
        let count_endpoint = format!(
            "{}&tags={}",
            self.active_imageboard.post_url(false).unwrap(),
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
        debug!("{} posts found", num);

        // In case the limit is set, use a whole other cascade of stuff
        if let Some(n) = self.download_limit {
            debug!("Using post limiter");
            // When it is set, we artificially set the counts to the limit in order to trick the progress bar.
            if n < num && n != 0 {
                self.item_count = n;
                self.page_count =
                    (n as f32 / self.active_imageboard.max_post_limit()).ceil() as usize;
            } else {
                debug!("Number of posts is lower than limit");
                self.item_count = num;
                self.page_count = (self.item_count as f32 / self.active_imageboard.max_post_limit())
                    .ceil() as usize;
            }
            // Or else, the usual
        } else {
            self.item_count = num;
            self.page_count =
                (self.item_count as f32 / self.active_imageboard.max_post_limit()).ceil() as usize;
        }

        // Gelbooru has a newer API that's a complete hell to decode in xml format, so we slightly change the url to the json endpoint.
        if self.active_imageboard == ImageBoards::Gelbooru {
            let count_endpoint = format!("{}&json=1", count_endpoint);
            self.posts_endpoint = count_endpoint;
            return Ok(());
        }

        self.posts_endpoint = count_endpoint;

        Ok(())
    }

    // This is mostly for sites running gelbooru 0.2, their xml API is way better than the JSON one
    fn generate_queue_xml(&self, xml: &str) -> Result<DownloadQueue, Error> {
        debug!("Using gelbooru XML API");
        let doc = Document::parse(xml)?;
        let stuff: Vec<Post> = doc
            .root_element()
            .children()
            .filter(|c| c.attribute("file_url").is_some())
            .map(|c| {
                let file = c.attribute("file_url").unwrap();
                let md5 = c.attribute("md5").unwrap().to_string();

                let link = if self.active_imageboard == ImageBoards::Realbooru {
                    let mut str: Vec<String> = file.split('/').map(|c| c.to_string()).collect();
                    str.pop();

                    format!("{}/{}.{}", str.join("/"), md5, extract_ext_from_url!(file))
                } else {
                    file.to_string()
                };

                Post {
                    id: c.attribute("id").unwrap().parse::<u64>().unwrap(),
                    url: link,
                    md5: c.attribute("md5").unwrap().to_string(),
                    extension: extract_ext_from_url!(file),
                    tags: Default::default(),
                }
            })
            .collect();

        debug!("Current queue size: {}", stuff.len());
        Ok(DownloadQueue::new(
            stuff,
            self.concurrent_downloads,
            self.download_limit,
            self.counters.clone(),
        ))
    }

    // This is for gelbooru.com itself, since it uses a new API that, while better on the JSON side, the XML part is an absolute hell to parse
    fn generate_queue_json(&self, json: &str) -> Result<DownloadQueue, Error> {
        debug!("Using gelbooru JSON API");
        let json: Value = serde_json::from_str(json)?;
        if let Some(it) = json["post"].as_array() {
            let posts: Vec<Post> = it
                .iter()
                .filter(|i| i["file_url"].as_str().is_some())
                .map(|post| {
                    let url = post["file_url"].as_str().unwrap().to_string();
                    Post {
                        id: post["id"].as_u64().unwrap(),
                        md5: post["md5"].as_str().unwrap().to_string(),
                        url: url.clone(),
                        extension: extract_ext_from_url!(url),
                        tags: Default::default(),
                    }
                })
                .collect();

            debug!("Current queue size: {}", posts.len());
            return Ok(DownloadQueue::new(
                posts,
                self.concurrent_downloads,
                self.download_limit,
                self.counters.clone(),
            ));
        }
        bail!("Failed to parse json")
    }

    pub async fn download(&mut self) -> Result<(), Error> {
        // Generate post count data
        Self::get_post_count(self).await?;

        // Create output dir
        create_dir_all(&self.out_dir).await?;

        // Setup global progress bar
        let bars = ProgressArcs::initialize(self.item_count as u64, self.active_imageboard);

        // Begin downloading all posts per page
        for i in 0..=self.page_count {
            bars.main.set_message(format!("Page {i}"));

            let items = &self
                .client
                .get(&self.posts_endpoint)
                .query(&[("pid", i), ("limit", 1000)])
                .send()
                .await?
                .text()
                .await?;

            let queue = if self.active_imageboard == ImageBoards::Gelbooru {
                Self::generate_queue_json(self, items)?
            } else {
                Self::generate_queue_xml(self, items)?
            };

            queue
                .download_post_list(
                    &self.client,
                    &self.out_dir,
                    bars.clone(),
                    self.active_imageboard,
                    self.save_as_id,
                )
                .await?;

            if let Some(n) = self.download_limit {
                if n == *self.counters.total_mtx.lock().unwrap() {
                    break;
                }
            }
        }

        finish_and_print_results!(bars, self);

        Ok(())
    }
}
