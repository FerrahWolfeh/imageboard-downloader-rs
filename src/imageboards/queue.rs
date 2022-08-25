use super::{blacklist::GlobalBlacklist, common::Counters, post::Post};
use crate::{progress_bars::ProgressArcs, ImageBoards};
use ahash::AHashSet;
use anyhow::Error;
use futures::StreamExt;
use log::debug;
use reqwest::Client;
use std::{path::Path, sync::Arc};

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
        let gbl = GlobalBlacklist::get().await?;

        if let Some(tags) = gbl.blacklist {
            debug!("Removing posts with tags [{:?}]", tags);
            Self::blacklist_filter(self, &tags);
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
                .retain(|c| c.tags.iter().any(|s| !blacklist.contains(s)));

            let bp = original_size - self.list.len();
            debug!("Removed {} blacklisted posts", bp);
            self.blacklisted += bp;
        }
    }

    pub fn blacklisted_ct(&self) -> usize {
        self.blacklisted
    }
}
