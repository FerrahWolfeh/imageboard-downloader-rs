//! Queue used specifically to download, filter and save posts found by an [Extractor](crate::imageboards::extractors).
//!
//! # Example usage
//!
//! ```rust
//! use imageboard_downloader::*;
//! use std::path::PathBuf;
//!
//! async fn download_posts() {
//!     let tags = ["umbreon".to_string(), "espeon".to_string()];
//!     
//!     let safe_mode = true; // Set to true to download posts from safebooru
//!
//!     let mut ext = DanbooruExtractor::new(&tags, safe_mode); // Initialize the extractor
//!
//!     ext.auth(false);
//!
//!     // Will iterate through all pages until it finds no more posts, then returns the list
//!     let posts = ext.full_search().await.unwrap();
//!
//!     let sd = 10; // Number of simultaneous downloads.
//!
//!     let limit = Some(1000); // Max number of posts to download
//!
//!     let cbz = false; // Set to true to download everything into a .cbz file
//!
//!     let mut qw = Queue::new( // Initialize the queue
//!         ImageBoards::Danbooru,
//!         posts,
//!         sd,
//!         limit,
//!         cbz,
//!     );
//!
//!     let output = Some(PathBuf::from("./")); // Where to save the downloaded files or .cbz file
//!
//!     let db = false; // Disable blacklist filtering
//!
//!     let id = true; // Save file with their ID as the filename instead of MD5
//!
//!     qw.download(output, db, id).await.unwrap(); // Start downloading
//! }
//! ```
use crate::imageboards::post::rating::Rating;
use crate::Post;
use crate::{client, progress_bars::ProgressCounter, ImageBoards};
use anyhow::Error;
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
use zip::write::FileOptions;
use zip::CompressionMethod;
use zip::ZipWriter;

use super::post::PostQueue;

/// Struct where all the downloading and filtering will take place
#[derive(Debug)]
pub struct Queue {
    list: Vec<Post>,
    tag_s: String,
    imageboard: ImageBoards,
    sim_downloads: usize,
    client: Client,
    limit: Option<usize>,
    cbz: bool,
}

impl Queue {
    /// Set up the queue for download
    pub fn new(
        imageboard: ImageBoards,
        posts: PostQueue,
        sim_downloads: usize,
        limit: Option<usize>,
        save_as_cbz: bool,
    ) -> Self {
        let st = posts.tags.join(" ");

        let client = client!(imageboard.user_agent());

        Self {
            list: posts.posts,
            tag_s: st,
            cbz: save_as_cbz,
            imageboard,
            sim_downloads,
            limit,
            client,
        }
    }

    /// Starts the download of all posts collected inside a [PostQueue]
    pub async fn download(
        &mut self,
        output: Option<PathBuf>,
        save_as_id: bool,
    ) -> Result<(), Error> {
        if let Some(max) = self.limit {
            let l_len = self.list.len();

            if max < l_len {
                self.list = self.list[0..max].to_vec();
            }
        }

        // If out_dir is not set via cli flags, use current dir
        let place = match output {
            None => std::env::current_dir()?,
            Some(dir) => dir,
        };

        let counters = ProgressCounter::initialize(self.list.len() as u64, self.imageboard);

        let mut task_pool = vec![];

        if self.cbz {
            let output_dir = place.join(PathBuf::from(self.imageboard.to_string()));

            debug!("Target file: {}/{}.cbz", output_dir.display(), self.tag_s);
            create_dir_all(&output_dir).await?;

            let output_file = output_dir.join(PathBuf::from(format!("{}.cbz", self.tag_s)));
            let oc = output_file.clone();

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
                    FileOptions::default()
                        .compression_method(CompressionMethod::Deflated)
                        .compression_level(Some(9)),
                )?;

                z_1.write_all(ap.as_bytes())?;
            }

            debug!("Fetching {} posts", self.list.len());

            for i in &self.list {
                let post = i.clone();
                let cli = self.client.clone();
                let output = oc.clone();
                let file = zip.clone();
                let imgbrd = self.imageboard;
                let counter = counters.clone();

                let task = tokio::task::spawn(async move {
                    post.get(&cli, &output, counter, imgbrd, save_as_id, file)
                        .await
                });
                task_pool.push(task);
            }

            futures::stream::iter(task_pool)
                .map(|d| d)
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

            for i in &self.list {
                let post = i.clone();
                let cli = self.client.clone();
                let output = output_dir.clone();
                let imgbrd = self.imageboard;
                let counter = counters.clone();

                let task = tokio::task::spawn(async move {
                    post.get(&cli, &output, counter, imgbrd, save_as_id, None)
                        .await
                });
                task_pool.push(task);
            }

            futures::stream::iter(task_pool)
                .map(|d| d)
                .buffer_unordered(self.sim_downloads)
                .collect::<Vec<_>>()
                .await;

            // debug!("Fetching {} posts", self.list.len());
            // futures::stream::iter(&self.list)
            //     .map(|d| {
            //         d.get(
            //             &self.client,
            //             &output_dir,
            //             counters.clone(),
            //             self.imageboard,
            //             save_as_id,
            //             None,
            //         )
            //     })
            //     .buffer_unordered(self.sim_downloads)
            //     .collect::<Vec<_>>()
            //     .await;
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

        // if removed > 0 && self.limit.is_none() {
        //     println!(
        //         "{} {}",
        //         removed.to_string().bold().red(),
        //         "posts with blacklisted tags were not downloaded."
        //             .bold()
        //             .red()
        //     )
        // }

        Ok(())
    }
}
