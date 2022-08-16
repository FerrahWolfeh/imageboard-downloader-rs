use crate::imageboards::common::{generate_out_dir, CommonPostItem};
use crate::imageboards::danbooru::models::{DanbooruItem, DanbooruPostCount};
use crate::progress_bars::master_progress_style;
use crate::{client, join_tags, AuthCredentials, ImageBoards};
use anyhow::{bail, Error};
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget};
use log::debug;
use reqwest::Client;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::create_dir_all;

pub mod models;

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
        let client = client!(ImageBoards::Danbooru.user_agent());

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

    async fn get_post_count(&mut self, auth_creds: &Option<AuthCredentials>) -> Result<(), Error> {
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
                .json::<DanbooruPostCount>()
                .await?
        } else {
            debug!("Fetching post count");
            self.client
                .get(count_endpoint)
                .send()
                .await?
                .json::<DanbooruPostCount>()
                .await?
        };

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
        // Get auth data
        let auth_res = AuthCredentials::read_from_fs(ImageBoards::Danbooru).await?;

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

        // Begin downloading all posts per page
        for i in 1..=self.page_count {
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
                    .json::<Vec<DanbooruItem>>()
                    .await?
            } else {
                debug!("Fetching posts from page {}", i);
                self.client
                    .get(url_mode)
                    .query(&[("page", &i.to_string()), ("limit", &200.to_string())])
                    .send()
                    .await?
                    .json::<Vec<DanbooruItem>>()
                    .await?
            };

            // Download everything got in the above function
            futures::stream::iter(jj)
                .map(|d| Self::download_item(self, d, multi.clone(), main.clone()))
                .buffer_unordered(self.concurrent_downloads)
                .collect::<Vec<_>>()
                .await;
        }
        main.finish_and_clear();
        Ok(())
    }

    async fn download_item(
        &self,
        item: DanbooruItem,
        multi_bar: Arc<MultiProgress>,
        main_bar: Arc<ProgressBar>,
    ) -> Result<(), Error> {
        if item.file_url.is_some() {
            let entity = CommonPostItem {
                url: item.file_url.unwrap(),
                md5: item.md5.unwrap(),
                ext: item.file_ext.unwrap(),
            };
            entity
                .get(
                    &self.client,
                    &self.out_dir,
                    multi_bar,
                    main_bar,
                    ImageBoards::Danbooru,
                )
                .await?;
            Ok(())
        } else {
            main_bar.set_length(main_bar.length().unwrap() - 1);
            Ok(())
        }
    }
}
