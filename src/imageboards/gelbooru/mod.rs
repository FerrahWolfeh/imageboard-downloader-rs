use crate::imageboards::common::{generate_out_dir, DownloadQueue, Post, ProgressArcs};
use crate::imageboards::ImageBoards;
use crate::progress_bars::master_progress_style;
use crate::{client, join_tags};
use crate::{extract_ext_from_url, print_results};
use anyhow::{bail, Error};
use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget};
use log::debug;
use reqwest::Client;
use roxmltree::Document;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
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
    downloaded_files: Arc<Mutex<u64>>,
}

impl GelbooruDownloader {
    pub fn new(
        imageboard: ImageBoards,
        tags: &[String],
        out_dir: Option<PathBuf>,
        concurrent_downs: usize,
        save_as_id: bool,
    ) -> Result<Self, Error> {
        // Use common client for all connections with a set User-Agent
        let client = client!(imageboard.user_agent());

        // Join tags to a url format in case there's more than one
        let tag_string = join_tags!(tags);

        // Place downloaded items in current dir or in /tmp
        let out = generate_out_dir(out_dir, &tag_string, imageboard)?;

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
            downloaded_files: Arc::new(Mutex::new(0)),
        })
    }

    async fn check_tag_list(&mut self) -> Result<(), Error> {
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

        // Fill memory with standard post count just to initialize the progress bar
        self.item_count = num;
        self.page_count =
            (self.item_count as f32 / self.active_imageboard.max_post_limit()).ceil() as usize;

        if self.active_imageboard == ImageBoards::Gelbooru {
            let count_endpoint = format!("{}&json=1", count_endpoint);
        self.posts_endpoint = count_endpoint;
        } else {
            self.posts_endpoint = count_endpoint;
        }

        Ok(())
    }

    fn generate_post_queue(&self, xml: &String) -> Result<Vec<Post>, Error> {
        if self.active_imageboard == ImageBoards::Gelbooru {
            let json: Value = serde_json::from_str(xml.as_str())?;
            if let Some(it) = json["post"].as_array() {
                let list: Vec<Post> = it
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
                return Ok(list);
            }
        }

        let doc = Document::parse(xml)?;
        let stuff: Vec<Post> = doc
            .root_element()
            .children()
            .filter(|c| c.attribute("file_url").is_some())
            .map(|c| {
                //c.children().map(|n| )
                let file = c.attribute("file_url").unwrap();
                let md5 = c.attribute("md5").unwrap().to_string();

                let link = if self.active_imageboard == ImageBoards::Realbooru {
                    let mut str: Vec<String> = file.split('/').map(|c| c.to_string()).collect();
                    str.pop();
                    //let modified = str.next().unwrap();
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
        Ok(stuff)
    }

    pub async fn download(&mut self) -> Result<(), Error> {
        // Generate post count data
        Self::check_tag_list(self).await?;

        // Create output dir
        create_dir_all(&self.out_dir).await?;

        // Setup global progress bar
        let bar = ProgressBar::new(self.item_count as u64).with_style(master_progress_style(
            &self.active_imageboard.progress_template(),
        ));
        bar.set_draw_target(ProgressDrawTarget::stderr_with_hz(60));
        bar.enable_steady_tick(Duration::from_millis(100));

        // Initialize the bars
        let multi = Arc::new(MultiProgress::new());
        let main = Arc::new(multi.add(bar));

        let bars = Arc::new(ProgressArcs { main, multi });

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

            let stuff = Self::generate_post_queue(&self, items)?;

            let queue = DownloadQueue::new(
                stuff,
                self.concurrent_downloads,
                self.downloaded_files.clone(),
            );
            queue
                .download_post_list(
                    &self.client,
                    &self.out_dir,
                    bars.clone(),
                    self.active_imageboard,
                    self.save_as_id,
                )
                .await?;
        }

        bars.main.finish_and_clear();

        print_results!(self);

        Ok(())
    }
}
