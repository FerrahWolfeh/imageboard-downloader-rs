//! Download and save `Post`s
use super::post::Post;
use crate::Rating;
use crate::{client, progress_bars::ProgressCounter, ImageBoards};
use ahash::AHashSet;
use anyhow::Error;
use cfg_if::cfg_if;
use colored::Colorize;
use futures::StreamExt;
use log::debug;
use reqwest::Client;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::fs::create_dir_all;
use tokio::time::Instant;
use zip::write::FileOptions;
use zip::CompressionMethod;
use zip::ZipWriter;

#[cfg(feature = "global_blacklist")]
use super::blacklist::GlobalBlacklist;

#[derive(Debug)]
pub struct PostQueue {
    pub posts: Vec<Post>,
    pub tags: Vec<String>,
    pub user_blacklist: AHashSet<String>,
}

#[derive(Debug)]
pub struct Queue {
    list: Vec<Post>,
    tag_s: String,
    imageboard: ImageBoards,
    sim_downloads: usize,
    client: Client,
    cbz: bool,
    user_blacklist: AHashSet<String>,
}

impl Queue {
    pub fn new(
        imageboard: ImageBoards,
        posts: PostQueue,
        sim_downloads: usize,
        limit: Option<usize>,
        save_as_cbz: bool,
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

        let client = client!(imageboard.user_agent());

        Self {
            list,
            tag_s: st,
            cbz: save_as_cbz,
            imageboard,
            sim_downloads,
            client,
            user_blacklist: posts.user_blacklist,
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

        let counters = ProgressCounter::initialize(self.list.len() as u64, self.imageboard);

        if self.cbz {
            let output_dir = place.join(PathBuf::from(self.imageboard.to_string()));

            debug!("Target file: {}/{}.cbz", output_dir.display(), self.tag_s);
            create_dir_all(&output_dir).await?;

            let output_file = output_dir.join(PathBuf::from(format!("{}.cbz", self.tag_s)));

            let zf = File::create(&output_file)?;
            let zip = Some(Arc::new(Mutex::new(ZipWriter::new(zf))));
            let z_mtx = zip.as_ref().unwrap();

            {
                let ap = serde_json::to_string_pretty(&self.list)?;

                let mut z_1 = z_mtx.lock().unwrap();
                z_1.add_directory(Rating::Safe.to_string(), Default::default())?;
                z_1.add_directory(Rating::Questionable.to_string(), Default::default())?;
                z_1.add_directory(Rating::Explicit.to_string(), Default::default())?;
                z_1.add_directory(Rating::Unknown.to_string(), Default::default())?;

                z_1.start_file(
                    "00_summary.json",
                    FileOptions::default().compression_method(CompressionMethod::Deflated),
                )?;

                z_1.write_all(ap.as_bytes())?;
            }

            debug!("Fetching {} posts", self.list.len());
            futures::stream::iter(&self.list)
                .map(|d| {
                    d.get(
                        &self.client,
                        &output_file,
                        counters.clone(),
                        self.imageboard,
                        save_as_id,
                        zip.clone(),
                    )
                })
                .buffer_unordered(self.sim_downloads)
                .collect::<Vec<_>>()
                .await;

            let mut zl = z_mtx.lock().unwrap();

            zl.set_comment(format!(
                "ImageBoard Downloader\n\nWebsite: {}\n\nTags: {}\n\nPosts: {}",
                self.imageboard.to_string(),
                self.tag_s,
                self.list.len()
            ));
            zl.finish()?;
        } else {
            let output_dir = place.join(PathBuf::from(format!(
                "{}/{}",
                self.imageboard.to_string(),
                self.tag_s
            )));

            debug!("Target dir: {}", output_dir.display());
            create_dir_all(&output_dir).await?;

            debug!("Fetching {} posts", self.list.len());
            futures::stream::iter(&self.list)
                .map(|d| {
                    d.get(
                        &self.client,
                        &output_dir,
                        counters.clone(),
                        self.imageboard,
                        save_as_id,
                        None,
                    )
                })
                .buffer_unordered(self.sim_downloads)
                .collect::<Vec<_>>()
                .await;
        }

        counters.main.finish_and_clear();
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
