//! Download and save `Post`s
use super::{common::Counters, post::Post};
use crate::{client, progress_bars::ProgressArcs, ImageBoards};
use ahash::AHashSet;
use anyhow::Error;
use cfg_if::cfg_if;
use colored::Colorize;
use futures::StreamExt;
use log::debug;
use reqwest::Client;
use std::path::PathBuf;
use std::sync::Mutex;
use std::{path::Path, sync::Arc};
use tokio::fs::create_dir_all;
use tokio::time::Instant;

#[cfg(feature = "global_blacklist")]
use super::blacklist::GlobalBlacklist;

#[derive(Debug)]
pub struct PostQueue {
    pub tags: Vec<String>,
    pub posts: Vec<Post>,
}

#[derive(Debug)]
pub struct Queue {
    list: Vec<Post>,
    tag_s: String,
    imageboard: ImageBoards,
    sim_downloads: usize,
    user_blacklist: AHashSet<String>,
}

impl Queue {
    pub fn new(
        imageboard: ImageBoards,
        posts: PostQueue,
        sim_downloads: usize,
        limit: Option<usize>,
        user_blacklist: AHashSet<String>,
    ) -> Self {
        let st = posts.tags.join(" ");

        let list = if let Some(max) = limit {
            let l_len = posts.posts.len();

            if max >= l_len {
                posts.posts
            } else {
                posts.posts[0..max].to_vec()
            }
        } else {
            posts.posts
        };

        Self {
            list,
            tag_s: st,
            imageboard,
            sim_downloads,
            user_blacklist,
        }
    }

    async fn blacklist_filter(&mut self, disable: bool) -> Result<u64, Error> {
        if disable {
            debug!("Blacklist filtering disabled");
            return Ok(0);
        }

        let original_size = self.list.len();
        let blacklist = &self.user_blacklist;
        let mut removed = 0;

        let start = Instant::now();
        if !blacklist.is_empty() {
            self.list
                .retain(|c| !c.tags.iter().any(|s| blacklist.contains(s)));

            let bp = original_size - self.list.len();
            debug!("User blacklist removed {} posts", bp);
            removed += bp as u64;
        }

        cfg_if! {
            if #[cfg(feature = "global_blacklist")] {
                let gbl = GlobalBlacklist::get().await?;

                if let Some(tags) = gbl.blacklist {
                    if !tags.global.is_empty() {
                        let fsize = self.list.len();
                        debug!("Removing posts with tags [{:?}]", tags);
                        self.list.retain(|c| !c.tags.iter().any(|s| tags.global.contains(s)));

                        let bp = fsize - self.list.len();
                        debug!("Global blacklist removed {} posts", bp);
                        removed += bp as u64;
                    } else {
                        debug!("Global blacklist is empty")
                    }

                    let special_tags = match self.imageboard {
                        ImageBoards::Danbooru => tags.danbooru,
                        ImageBoards::E621 => tags.e621,
                        ImageBoards::Rule34 => tags.rule34,
                        ImageBoards::Realbooru => tags.realbooru,
                        ImageBoards::Konachan => tags.konachan,
                        ImageBoards::Gelbooru => tags.gelbooru,
                    };

                    if !special_tags.is_empty() {
                        let fsize = self.list.len();
                        debug!("Removing posts with tags [{:?}]", special_tags);
                        self.list.retain(|c| !c.tags.iter().any(|s| special_tags.contains(s)));

                        let bp = fsize - self.list.len();
                        debug!("Danbooru blacklist removed {} posts", bp);
                        removed += bp as u64;
                    }
                }
            }
        }
        let end = Instant::now();
        debug!("Blacklist filtering took {:?}", end - start);
        debug!("Removed {} blacklisted posts", removed);

        Ok(removed)
    }

    pub async fn download(
        &mut self,
        output: Option<PathBuf>,
        disable_blacklist: bool,
        save_as_id: bool,
    ) -> Result<(), Error> {
        let removed = Self::blacklist_filter(self, disable_blacklist).await?;

        // If out_dir is not set via cli flags, use current dir
        let place = match output {
            None => std::env::current_dir()?,
            Some(dir) => dir,
        };

        let output_dir = place.join(PathBuf::from(format!(
            "{}/{}",
            self.imageboard.to_string(),
            self.tag_s
        )));

        debug!("Target dir: {}", output_dir.display());
        create_dir_all(&output_dir).await?;

        let bars = ProgressArcs::initialize(self.list.len() as u64, self.imageboard);

        let counters = Arc::new(Counters {
            total_mtx: Arc::new(Mutex::new(0)),
            downloaded_mtx: Arc::new(Mutex::new(0)),
        });

        let client = client!(self.imageboard.user_agent());

        debug!("Fetching {} posts", self.list.len());
        futures::stream::iter(&self.list)
            .map(|d| {
                d.get(
                    &client,
                    &output_dir,
                    bars.clone(),
                    self.imageboard,
                    counters.clone(),
                    save_as_id,
                )
            })
            .buffer_unordered(self.sim_downloads)
            .collect::<Vec<_>>()
            .await;

        bars.main.finish_and_clear();
        println!(
            "{} {} {}",
            counters
                .downloaded_mtx
                .lock()
                .unwrap()
                .to_string()
                .bold()
                .blue(),
            "files".bold().blue(),
            "downloaded".bold()
        );

        if removed > 0 {
            println!(
                "{} {}",
                removed.to_string().bold().red(),
                "posts with blacklisted tags were not downloaded."
                    .bold()
                    .red()
            )
        }

        Ok(())
    }
}

/// # Download Queue
/// Aggregates all posts and downloads them simultaneously according supplied parameters.
pub struct DownloadQueue {
    list: Vec<Post>,
    concurrent_downloads: usize,
    counters: Arc<Counters>,
    blacklisted: usize,
}

impl DownloadQueue {
    pub fn new(
        list: Vec<Post>,
        concurrent_downloads: usize,
        limit: Option<usize>,
        counters: Counters,
    ) -> Self {
        let list = if let Some(max) = limit {
            let dt = *counters.total_mtx.lock().unwrap();
            let l_len = list.len();
            let ran = max - dt;

            if ran >= l_len {
                list
            } else {
                list[0..ran].to_vec()
            }
        } else {
            list
        };

        Self {
            list,
            concurrent_downloads,
            counters: Arc::new(counters),
            blacklisted: 0,
        }
    }

    pub async fn download(
        &mut self,
        client: &Client,
        output_dir: &Path,
        bars: Arc<ProgressArcs>,
        variant: ImageBoards,
        save_as_id: bool,
    ) -> Result<(), Error> {
        cfg_if! {
            if #[cfg(feature = "global_blacklist")] {
                let gbl = GlobalBlacklist::get().await?;

                if let Some(tags) = gbl.blacklist {
                    debug!("Removing posts with tags [{:?}]", tags);
                    Self::blacklist_filter(self, &tags.global);
                }
            }
        }

        debug!("Fetching {} posts", self.list.len());
        futures::stream::iter(&self.list)
            .map(|d| {
                d.get(
                    client,
                    output_dir,
                    bars.clone(),
                    variant,
                    self.counters.clone(),
                    save_as_id,
                )
            })
            .buffer_unordered(self.concurrent_downloads)
            .collect::<Vec<_>>()
            .await;

        Ok(())
    }

    pub fn blacklist_filter(&mut self, blacklist: &AHashSet<String>) {
        let original_size = self.list.len();

        if !blacklist.is_empty() {
            self.list
                .retain(|c| !c.tags.iter().any(|s| blacklist.contains(s)));

            let bp = original_size - self.list.len();
            debug!("Removed {} blacklisted posts", bp);
            self.blacklisted += bp;
        }
    }

    pub fn blacklisted_ct(&self) -> usize {
        self.blacklisted
    }
}
