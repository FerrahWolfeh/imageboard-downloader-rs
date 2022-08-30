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
use tokio::time::Instant;
use zip::write::FileOptions;
use zip::CompressionMethod;
use zip::ZipWriter;

use super::post::PostQueue;

/// Struct where all the downloading and filtering will take place
pub struct Queue {
    list: Vec<Post>,
    tag_s: String,
    imageboard: ImageBoards,
    sim_downloads: usize,
    client: Client,
    limit: Option<usize>,
    cbz: bool,
    zip_file: Option<Arc<Mutex<ZipWriter<File>>>>,
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
            zip_file: None,
        }
    }

    /// Starts the download of all posts collected inside a [PostQueue]
    pub async fn download(
        &mut self,
        output: Option<PathBuf>,
        save_as_id: bool,
    ) -> Result<(), Error> {
        let fstart = Instant::now();

        if let Some(max) = self.limit {
            let l_len = self.list.len();

            if max < l_len {
                self.list = self.list[0..max].to_vec();
            }
        }

        self.list.sort();

        self.list.reverse();

        let fend = Instant::now();

        debug!("List final sorting took {:?}", fend - fstart);

        // If out_dir is not set via cli flags, use current dir
        let place = match output {
            None => std::env::current_dir()?,
            Some(dir) => dir,
        };

        let counters = ProgressCounter::initialize(self.list.len() as u64, self.imageboard);

        let mut task_pool = vec![];

        if self.cbz {
            let output_file = place.join(PathBuf::from(format!(
                "{}/{}.cbz",
                self.imageboard.to_string(),
                self.tag_s
            )));

            let oc = output_file.clone();

            debug!("Target file: {}", output_file.display());
            create_dir_all(&output_file.parent().unwrap()).await?;

            let zf = File::create(&output_file)?;
            let zip = Some(Arc::new(Mutex::new(ZipWriter::new(zf))));
            self.zip_file = Some(zip.unwrap());

            let zf = self.zip_file.clone().unwrap();

            {
                let ap = serde_json::to_string_pretty(&self.list)?;

                let mut z_1 = zf.lock().unwrap();
                z_1.set_comment(format!(
                    "ImageBoard Downloader\n\nWebsite: {}\n\nTags: {}\n\nPosts: {}",
                    self.imageboard.to_string(),
                    self.tag_s,
                    self.list.len()
                ));

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

            for i in &self.list {
                let post = i.clone();
                let cli = self.client.clone();
                let output = oc.clone();
                let file = zf.clone();
                let imgbrd = self.imageboard;
                let counter = counters.clone();

                let task = tokio::task::spawn(async move {
                    post.get(&cli, &output, counter, imgbrd, save_as_id, Some(file))
                        .await
                });
                task_pool.push(task);
            }
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
        }

        debug!("Fetching {} posts", self.list.len());

        futures::stream::iter(task_pool)
            .map(|d| d)
            .buffer_unordered(self.sim_downloads)
            .collect::<Vec<_>>()
            .await;

        if self.cbz {
            let file = self.zip_file.as_ref().unwrap();
            let mut mtx = file.lock().unwrap();

            mtx.finish()?;
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
