//! Provides the core asynchronous download queue and related logic.
//!
//! This module contains the [`Queue`](crate::async_queue::Queue) struct, which is the central component for managing
//! the download process. It receives `Post` objects, handles
//! concurrent downloads, saves files to disk (either in folders or CBZ archives),
//! and reports progress via a `ProgressListener`.
//!
//! The download process is designed to be driven by an external source (like an extractor)
//! that sends [`Post`](ibdl_common::post::Post) objects through a channel.

#[cfg(feature = "cbz")]
mod cbz;

// Contains the logic for downloading and saving files to a directory.
mod folder;

use crate::error::DownloaderError;
// Import the new progress listener traits and helpers
use crate::progress::{SharedProgressListener, no_op_progress_listener};
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

/// Specifies the output format for downloaded files.
///
/// This enum determines whether files are saved into a CBZ archive or a regular folder,
/// and whether the download pertains to a pool (which might affect naming or structure).
/// Variants requiring the `cbz` feature are conditionally compiled.
#[derive(Debug, Copy, Clone)]
enum DownloadFormat {
    /// Download files into a CBZ archive. Requires the `cbz` feature.
    #[cfg(feature = "cbz")]
    Cbz,
    /// Download files belonging to a pool into a CBZ archive. Requires the `cbz` feature.
    #[cfg(feature = "cbz")]
    CbzPool,
    /// Download files into a regular directory structure.
    Folder,
    /// Download files belonging to a pool into a regular directory structure.
    FolderPool,
}

impl DownloadFormat {
    /// Checks if the current format involves creating a CBZ archive.
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

    /// Checks if the current format is for a pool download.
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
    /// If `true`, downloaded files will be saved into a `.cbz` archive.
    /// This requires the `cbz` feature to be enabled. If `false`, files are saved to a folder.
    pub save_as_cbz: bool,
    /// If `true`, indicates that the download is for a pool of posts.
    /// This can affect the naming of the output CBZ file or directory.
    pub pool_download: bool,
    /// Specifies the naming convention for downloaded files (e.g., using post ID, MD5 hash).
    pub name_type: NameType,
    /// If `true`, a text file with a list of tags (captions) will be created for each downloaded image.
    pub annotate: bool,
}

/// Manages the asynchronous download of posts.
///
/// The `Queue` is responsible for receiving `Post` objects,
/// coordinating concurrent downloads, saving files to the specified format (folder or CBZ),
/// and reporting progress.
pub struct Queue {
    /// The number of concurrent downloads allowed.
    sim_downloads: u8,
    /// The `reqwest::Client` used for making HTTP requests.
    client: Client,
    /// The determined output format (CBZ, Folder, etc.) based on `QueueOpts` and feature flags.
    download_fmt: DownloadFormat,
    /// The naming convention for downloaded files.
    name_type: NameType,
    /// Whether to generate annotation/caption files for posts.
    annotate: bool,
    /// A shared progress listener for reporting download progress.
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
            // If no custom client is provided, create a new one using the server_config.
            Client::builder()
                .user_agent(server_config.client_user_agent)
                .build()
                .unwrap()
        };

        let download_fmt = if options.pool_download {
            // Determine format for pool downloads
            #[cfg(feature = "cbz")]
            if options.save_as_cbz {
                DownloadFormat::CbzPool
            } else {
                DownloadFormat::FolderPool
            }
            #[cfg(not(feature = "cbz"))]
            // If CBZ feature is disabled, pool download always goes to a folder.
            DownloadFormat::FolderPool
        } else {
            // Determine format for non-pool (tag search) downloads
            // Currently, non-pool downloads always go to a folder, CBZ for tags is handled by output path generation.
            DownloadFormat::Folder
        };

        let listener = progress_listener.unwrap_or_else(no_op_progress_listener); // Use a no-op listener if none is provided.
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
    /// # Behavior
    /// This function spawns a new asynchronous task that listens on `channel_rx` for `Post` objects.
    /// It then proceeds to download these posts according to the `Queue`'s configuration
    /// (e.g., number of simultaneous downloads, output format).
    ///
    /// Progress is reported via the `progress_listener` provided during `Queue::new`.
    /// The main progress bar (controlled by `progress_listener.main_tick()` and `set_main_total()`)
    /// tracks posts as they are received from the `channel_rx`. The total for this bar should be
    /// set by the caller after determining the total number of posts to expect.
    ///
    /// The caller is responsible for:
    /// 1. Creating and configuring the `SharedProgressListener` (passed during `Queue::new`).
    /// 2. Calling `set_main_total` on the `ProgressListener` once the total number of posts is known.
    /// 3. Sending `Post` objects into `channel_rx` and closing the sender when done.
    ///
    /// # Returns
    /// A `JoinHandle` to the spawned task. The task itself returns a `Result<u64, DownloaderError>`,
    /// where `u64` is the total number of successfully downloaded posts.
    pub fn setup_async_downloader(
        self,
        output_dir: PathBuf,
        channel_rx: UnboundedReceiver<Post>,
    ) -> JoinHandle<Result<u64, DownloaderError>> {
        let progress_listener = self.progress_listener.clone(); // Clone Arc for the spawned task

        spawn(async move {
            debug!("Async Downloader thread initialized");

            // Create the output directory structure.
            self.create_out(&output_dir).await?;

            let post_channel = UnboundedReceiverStream::new(channel_rx);

            let downloaded_post_count = Arc::new(AtomicU64::new(0));
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

            // Signal that all posts from the channel have been processed.
            self.progress_listener.main_done();

            let total_downloaded = downloaded_post_count.load(Ordering::SeqCst);
            Ok(total_downloaded)
        })
    }

    /// Creates the necessary output directory structure.
    ///
    /// If downloading to a CBZ file, this creates the parent directory of the CBZ file.
    /// If downloading to a folder, this creates the target folder itself.
    ///
    /// # Arguments
    /// * `dir`: The path to the output file (for CBZ) or directory (for folder).
    async fn create_out(&self, dir: &Path) -> Result<(), DownloaderError> {
        if self.download_fmt.download_cbz() {
            let output_file = dir.parent().unwrap().to_path_buf();

            match create_dir_all(&output_file).await {
                Ok(_) => (),
                Err(error) => {
                    return Err(DownloaderError::DirCreationError {
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
                return Err(DownloaderError::DirCreationError {
                    message: error.to_string(),
                });
            }
        };

        Ok(())
    }

    /// Writes a caption file for a given post.
    ///
    /// The caption file is a text file named after the post (using `name_type`) with a `.txt` extension.
    /// It contains a comma-separated list of the post's tags that are considered "prompt tags"
    /// (see `Tag::is_prompt_tag`), with underscores replaced by spaces.
    ///
    /// # Arguments
    /// * `post`: The `Post` for which to write the caption.
    /// * `name_type`: The naming convention to use for the caption file.
    /// * `output`: The directory where the caption file will be saved.
    async fn write_caption(
        post: &Post,
        name_type: NameType,
        output: &Path,
    ) -> Result<(), DownloaderError> {
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
}
