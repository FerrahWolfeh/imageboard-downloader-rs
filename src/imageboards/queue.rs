use super::{common::Counters, post::Post};
use crate::{progress_bars::ProgressArcs, ImageBoards};
use anyhow::Error;
use futures::StreamExt;
use log::debug;
use reqwest::Client;
use std::{path::Path, sync::Arc};

pub struct DownloadQueue {
    pub list: Vec<Post>,
    pub concurrent_downloads: usize,
    pub counters: Arc<Counters>,
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
        }
    }

    pub async fn download_all(
        self,
        client: &Client,
        output_dir: &Path,
        bars: Arc<ProgressArcs>,
        variant: ImageBoards,
        save_as_id: bool,
    ) -> Result<(), Error> {
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
}
