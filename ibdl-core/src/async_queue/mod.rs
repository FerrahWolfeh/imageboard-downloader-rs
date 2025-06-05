//! Queue used specifically to download, filter and save posts found by an [`Extractor`](ibdl-extractors::websites).
//!
//! # Example usage
//!
//! Conveniently using the same example from [here](ibdl-extractors::websites)
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

#[cfg(feature = "cbz")]
mod cbz;

mod folder;

use crate::error::QueueError;
// Import the new progress listener traits and helpers
use crate::progress::{SharedProgressListener, no_op_progress_listener};
use ibdl_common::post::error::PostError;
use ibdl_common::post::{NameType, Post};
use ibdl_extractors::extractor_config::ServerConfig;
use log::debug;
use reqwest::Client;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs::{OpenOptions, create_dir_all};
use tokio::io::AsyncWriteExt;
use tokio::spawn;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::UnboundedReceiverStream;

#[derive(Debug, Copy, Clone)]
enum DownloadFormat {
    #[cfg(feature = "cbz")]
    Cbz,
    #[cfg(feature = "cbz")]
    CbzPool,
    Folder,
    FolderPool,
}

impl DownloadFormat {
    #[inline]
    pub const fn download_cbz(&self) -> bool {
        match self {
            #[cfg(feature = "cbz")]
            Self::Cbz => true,
            #[cfg(feature = "cbz")]
            Self::CbzPool => true,
            #[cfg(not(feature = "cbz"))] // If cbz feature is off, these variants don't exist
            Self::Folder => false,
            Self::FolderPool => false,
            #[cfg(feature = "cbz")]
            _ => false,
        }
    }

    #[inline]
    pub const fn download_pool(&self) -> bool {
        match self {
            #[cfg(feature = "cbz")]
            Self::Cbz => false,
            #[cfg(feature = "cbz")]
            Self::CbzPool => true,
            Self::Folder => false,
            Self::FolderPool => true,
        }
    }
}

/// Options for configuring the output and naming of downloaded files.
#[derive(Debug, Clone, Copy)]
pub struct QueueOpts {
    pub save_as_cbz: bool,
    pub pool_download: bool,
    pub name_type: NameType,
    pub annotate: bool,
}

/// Struct where all the downloading will take place
pub struct Queue {
    sim_downloads: u8,
    client: Client,
    download_fmt: DownloadFormat,
    name_type: NameType,
    annotate: bool,
    // No imageboard field here, it's used for client creation only if needed
    progress_listener: SharedProgressListener,
}

impl Queue {
    /// Set up the queue for download
    pub fn new(
        server_config: ServerConfig,
        sim_downloads: u8,
        custom_client: Option<Client>,
        options: QueueOpts,
        progress_listener: Option<SharedProgressListener>,
    ) -> Self {
        let client = if let Some(cli) = custom_client {
            cli
        } else {
            Client::builder()
                .user_agent(server_config.client_user_agent)
                .build()
                .unwrap()
        };

        let download_fmt = if options.pool_download {
            #[cfg(feature = "cbz")]
            if options.save_as_cbz {
                DownloadFormat::CbzPool
            } else {
                DownloadFormat::FolderPool
            }
            #[cfg(not(feature = "cbz"))]
            // If CBZ is disabled, pool download always goes to folder
            DownloadFormat::FolderPool
        } else {
            DownloadFormat::Folder
        };

        let listener = progress_listener.unwrap_or_else(no_op_progress_listener);
        Self {
            download_fmt,
            sim_downloads,
            annotate: options.annotate,
            client,
            name_type: options.name_type,
            progress_listener: listener,
        }
    }

    /// Spawns the main asynchronous download task.
    ///
    /// # Arguments
    /// * `output_dir`: The base directory where files or CBZ archives will be saved.
    /// * `channel_rx`: An unbounded receiver for `Post` objects to be downloaded.
    ///
    /// The caller is responsible for:
    /// 1. Creating and configuring the `ProgressListener` (passed during `Queue::new`).
    /// 2. Calling `set_main_total` and `inc_main_total` on the `ProgressListener` as posts are discovered.
    /// 3. Sending `Post` objects into `channel_rx`.
    ///
    /// # Returns
    /// A `JoinHandle` to the spawned task, which will return the total number of successfully
    /// downloaded posts or a `QueueError`.
    pub fn setup_async_downloader(
        self,
        output_dir: PathBuf,
        channel_rx: UnboundedReceiver<Post>,
    ) -> JoinHandle<Result<u64, QueueError>> {
        let progress_listener = self.progress_listener.clone(); // Clone Arc for the spawned task

        spawn(async move {
            debug!("Async Downloader thread initialized");

            self.create_out(&output_dir).await?;

            let post_channel = UnboundedReceiverStream::new(channel_rx);

            // Counter for successfully downloaded posts
            // This counter tracks posts that are fully downloaded and saved.
            let downloaded_post_count = Arc::new(AtomicU64::new(0));

            // The main progress bar (controlled by progress_listener.main_tick() and set_main_total())
            // will now track posts as they are received from the extractor into the queue.
            // The total for this bar is set by the caller (e.g., main.rs) after the extractor
            // sends the total number of posts it expects to fetch.
            // The condition now correctly checks if the 'cbz' feature is enabled AND
            // if the download format is actually CBZ.
            if cfg!(feature = "cbz") && self.download_fmt.download_cbz() {
                #[cfg(feature = "cbz")]
                self.cbz_path(
                    output_dir,   // This is the CBZ file path itself
                    post_channel, // Stream of posts from the extractor
                    self.download_fmt.download_pool(),
                    progress_listener.clone(), // Pass listener to internal methods
                    downloaded_post_count.clone(), // Pass counter
                )
                .await?;
            } else {
                // This branch is taken if 'cbz' feature is disabled,
                // OR if 'cbz' feature is enabled but folder download is selected.
                self.download_channel(
                    post_channel,
                    output_dir, // This is the root directory for downloaded files
                    self.download_fmt.download_pool(),
                    progress_listener.clone(), // Pass listener to internal methods
                    downloaded_post_count.clone(), // Pass counter
                )
                .await;
            }

            // Signal that the main processing is done via the listener
            // This will finish the main progress bar, indicating all posts received
            // from the extractor have been processed (attempted for download).
            self.progress_listener.main_done();

            let total_downloaded = downloaded_post_count.load(Ordering::SeqCst);
            Ok(total_downloaded)
        })
    }

    async fn create_out(&self, dir: &Path) -> Result<(), QueueError> {
        if self.download_fmt.download_cbz() {
            let output_file = dir.parent().unwrap().to_path_buf();

            match create_dir_all(&output_file).await {
                Ok(_) => (),
                Err(error) => {
                    return Err(QueueError::DirCreationError {
                        message: error.to_string(),
                    });
                }
            };
            return Ok(());
        }

        debug!("Target dir: {}", dir.display());
        match create_dir_all(&dir).await {
            Ok(_) => (),
            Err(error) => {
                return Err(QueueError::DirCreationError {
                    message: error.to_string(),
                });
            }
        };

        Ok(())
    }

    async fn write_caption(
        post: &Post,
        name_type: NameType,
        output: &Path,
    ) -> Result<(), PostError> {
        let outpath = output.join(format!("{}.txt", post.name(name_type)));
        let mut prompt_file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(outpath)
            .await?;

        let tag_list = Vec::from_iter(
            post.tags
                .iter()
                .filter(|t| t.is_prompt_tag())
                .map(|tag| tag.tag()),
        );

        let prompt = tag_list.join(", ");

        let f1 = prompt.replace('_', " ");

        prompt_file.write_all(f1.as_bytes()).await?;
        debug!("Wrote caption file for {}", post.file_name(name_type));
        Ok(())
    }

    // NOTE on Progress Handling:
    //
    // The `SharedProgressListener` is used for two main types of progress:
    // 1. Main Progress (Extractor Output):
    //    - Total: Set by the CLI (`main.rs`) when the extractor communicates the total number of posts it will fetch.
    //             This happens via the `length_tx` channel passed to the extractor.
    //    - Ticks: `progress_listener.main_tick()` is called within `cbz_path` or `download_channel`
    //             as soon as a `Post` is received from the `post_channel` (i.e., from the extractor).
    // 2. Download Progress (Per-File):
    //    - Handled by `progress_listener.add_download_task(...)` and its associated `DownloadProgressUpdater` methods.
    //    - `downloaded_post_count` (Arc<AtomicU64>) is incremented after a post's file is successfully downloaded.
}
