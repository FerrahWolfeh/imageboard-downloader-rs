//! Queue used specifically to download, filter and save posts found by an [`Extractor`](crate::imageboards::extractors).
//!
//! # Example usage
//!
//! Conveniently using the same example from [here](crate::imageboards::extractors)
//!
//! ```rust
//! use imageboard_downloader::*;
//! use std::path::PathBuf;
//!
//! async fn download_posts() {
//!     let tags = ["umbreon", "espeon"]; // The tags to search
//!     
//!     let safe_mode = false; // Setting this to true, will ignore searching NSFW posts
//!
//!     let disable_blacklist = false; // Will filter all items according to what's set in GBL
//!
//!     let mut unit = DanbooruExtractor::new(&tags, safe_mode, disable_blacklist); // Initialize
//!
//!     let prompt = true; // If true, will ask the user to input thei username and API key.
//!
//!     unit.auth(prompt).await.unwrap(); // Try to authenticate
//!
//!     let start_page = Some(1); // Start searching from the first page
//!
//!     let limit = Some(50); // Max number of posts to download
//!
//!     let posts = unit.full_search(start_page, limit).await.unwrap(); // and then, finally search
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
//!         Some(unit.client()), // Re-use the client from the extractor
//!         limit,
//!         cbz,
//!     );
//!
//!     let output = Some(PathBuf::from("./")); // Where to save the downloaded files or .cbz file
//!
//!     let id = true; // Save file with their ID as the filename instead of MD5
//!
//!     qw.download(output, id).await.unwrap(); // Start downloading
//! }
//! ```
use crate::imageboards::post::rating::Rating;
use crate::Post;
use crate::{client, progress_bars::ProgressCounter, ImageBoards};
use futures::StreamExt;
use log::debug;
use reqwest::Client;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::fs::create_dir_all;
use tokio::task;
use zip::write::FileOptions;
use zip::CompressionMethod;
use zip::ZipWriter;

use self::error::QueueError;

use super::post::PostQueue;

mod error;

/// Struct where all the downloading and filtering will take place
pub struct Queue {
    list: Vec<Post>,
    tag_s: String,
    imageboard: ImageBoards,
    sim_downloads: u8,
    client: Client,
    cbz: bool,
    zip_file: Option<Arc<Mutex<ZipWriter<File>>>>,
}

impl Queue {
    /// Set up the queue for download
    #[must_use]
    pub fn new(
        imageboard: ImageBoards,
        posts: PostQueue,
        sim_downloads: u8,
        custom_client: Option<Client>,
        limit: Option<usize>,
        save_as_cbz: bool,
    ) -> Self {
        let st = posts.tags.join(" ");

        let client = if let Some(cli) = custom_client {
            cli
        } else {
            client!(imageboard)
        };

        let mut plist = posts.posts;

        if let Some(max) = limit {
            let l_len = plist.len();

            if max < l_len {
                plist = plist[0..max].to_vec();
            }
        }

        Self {
            list: plist,
            tag_s: st,
            cbz: save_as_cbz,
            imageboard,
            sim_downloads,
            client,
            zip_file: None,
        }
    }

    /// Starts the download of all posts collected inside a [`PostQueue`]
    pub async fn download(
        &mut self,
        output: Option<PathBuf>,
        save_as_id: bool,
    ) -> Result<u64, QueueError> {
        if self.list.is_empty() {
            return Err(QueueError::NoPostsInQueue);
        }

        // If out_dir is not set via cli flags, use current dir
        let place = match output {
            None => match std::env::current_dir() {
                Ok(result) => result,
                Err(reason) => return Err(QueueError::EnvError { source: reason }),
            },
            Some(dir) => dir,
        };

        let counters = ProgressCounter::initialize(self.list.len() as u64, self.imageboard);

        let output_place = if self.cbz {
            let output_file = place.join(PathBuf::from(format!(
                "{}/{}.cbz",
                self.imageboard.to_string(),
                self.tag_s
            )));

            debug!("Target file: {}", output_file.display());
            match create_dir_all(&output_file.parent().unwrap()).await {
                Ok(_) => (),
                Err(error) => {
                    return Err(QueueError::DirCreationError {
                        message: error.to_string(),
                    })
                }
            };
            output_file
        } else {
            let output_dir = place.join(PathBuf::from(format!(
                "{}/{}",
                self.imageboard.to_string(),
                self.tag_s
            )));

            debug!("Target dir: {}", output_dir.display());
            match create_dir_all(&output_dir).await {
                Ok(_) => (),
                Err(error) => {
                    return Err(QueueError::DirCreationError {
                        message: error.to_string(),
                    })
                }
            };

            output_dir
        };

        if self.cbz {
            let output_file = place.join(PathBuf::from(format!(
                "{}/{}.cbz",
                self.imageboard.to_string(),
                self.tag_s
            )));

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

                z_1.add_directory(Rating::Safe.to_string(), FileOptions::default())?;
                z_1.add_directory(Rating::Questionable.to_string(), FileOptions::default())?;
                z_1.add_directory(Rating::Explicit.to_string(), FileOptions::default())?;
                z_1.add_directory(Rating::Unknown.to_string(), FileOptions::default())?;

                z_1.start_file(
                    "00_summary.json",
                    FileOptions::default()
                        .compression_method(CompressionMethod::Deflated)
                        .compression_level(Some(9)),
                )?;

                z_1.write_all(ap.as_bytes())?;
            }
        }

        debug!("Fetching {} posts", self.list.len());

        futures::stream::iter(&self.list)
            .map(|d| {
                let post = d.clone();
                let cli = self.client.clone();
                let output = output_place.clone();
                let imgbrd = self.imageboard;
                let counter = counters.clone();
                let selfe = self.zip_file.clone();

                task::spawn(async move {
                    post.get(&cli, &output, counter, imgbrd, save_as_id, selfe)
                        .await
                })
            })
            .buffer_unordered(self.sim_downloads as usize)
            .collect::<Vec<_>>()
            .await;

        if self.cbz {
            let file = self.zip_file.as_ref().unwrap();
            let mut mtx = file.lock().unwrap();

            mtx.finish()?;
        }

        counters.main.finish_and_clear();

        let tot = counters.downloaded_mtx.lock().unwrap();

        Ok(*tot)
    }
}
