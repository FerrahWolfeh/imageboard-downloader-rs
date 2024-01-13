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
//mod summary;

mod cbz;
mod folder;

use crate::error::QueueError;
use crate::progress_bars::ProgressCounter;
use ibdl_common::log::debug;
use ibdl_common::post::error::PostError;
use ibdl_common::post::{NameType, Post};
use ibdl_common::reqwest::Client;
use ibdl_common::tokio::spawn;
use ibdl_common::tokio::sync::mpsc::{channel, Receiver, UnboundedReceiver};
use ibdl_common::tokio::task::JoinHandle;
use ibdl_common::{client, tokio};
use ibdl_extractors::extractor_config::ServerConfig;
use once_cell::sync::OnceCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::fs::{create_dir_all, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio_stream::wrappers::UnboundedReceiverStream;

static PROGRESS_COUNTERS: OnceCell<ProgressCounter> = OnceCell::new();

pub(crate) fn get_counters() -> &'static ProgressCounter {
    PROGRESS_COUNTERS.get().unwrap()
}

#[derive(Debug, Copy, Clone)]
enum DownloadFormat {
    Cbz,
    CbzPool,
    Folder,
    FolderPool,
}

impl DownloadFormat {
    #[inline]
    pub const fn download_cbz(&self) -> bool {
        match self {
            Self::Cbz => true,
            Self::CbzPool => true,
            Self::Folder => false,
            Self::FolderPool => false,
        }
    }

    #[inline]
    pub const fn download_pool(&self) -> bool {
        match self {
            Self::Cbz => false,
            Self::CbzPool => true,
            Self::Folder => false,
            Self::FolderPool => true,
        }
    }
}

/// Struct where all the downloading will take place
pub struct Queue {
    imageboard: ServerConfig,
    sim_downloads: u8,
    client: Client,
    download_fmt: DownloadFormat,
    name_type: NameType,
    annotate: bool,
}

impl Queue {
    /// Set up the queue for download
    pub fn new(
        imageboard: ServerConfig,
        sim_downloads: u8,
        custom_client: Option<Client>,
        save_as_cbz: bool,
        pool_download: bool,
        name_type: NameType,
        annotate: bool,
    ) -> Self {
        let client = if let Some(cli) = custom_client {
            cli
        } else {
            client!(imageboard)
        };

        let download_fmt = if save_as_cbz && pool_download {
            DownloadFormat::CbzPool
        } else if save_as_cbz {
            DownloadFormat::Cbz
        } else if pool_download {
            DownloadFormat::FolderPool
        } else {
            DownloadFormat::Folder
        };

        Self {
            download_fmt,
            imageboard,
            sim_downloads,
            annotate,
            client,
            name_type,
        }
    }

    pub fn setup_async_downloader(
        self,
        output_dir: PathBuf,
        post_counter: Arc<AtomicU64>,
        channel_rx: UnboundedReceiver<Post>,
        length_rx: Receiver<u64>,
    ) -> JoinHandle<Result<u64, QueueError>> {
        spawn(async move {
            debug!("Async Downloader thread initialized");

            let counters = PROGRESS_COUNTERS.get_or_init(|| {
                ProgressCounter::initialize(
                    post_counter.load(Ordering::Relaxed),
                    self.imageboard.server,
                )
            });

            self.create_out(&output_dir).await?;

            let post_channel = UnboundedReceiverStream::new(channel_rx);
            let (progress_sender, progress_channel) = channel(self.sim_downloads as usize);

            counters.init_length_updater(length_rx).await;
            counters.init_download_counter(progress_channel).await;

            if self.download_fmt.download_cbz() {
                self.cbz_path(
                    output_dir,
                    progress_sender,
                    post_channel,
                    self.download_fmt.download_pool(),
                )
                .await?;
            } else {
                self.download_channel(
                    post_channel,
                    progress_sender,
                    output_dir,
                    self.download_fmt.download_pool(),
                )
                .await;
            }

            counters.main.finish_and_clear();

            let tot = counters.downloaded_mtx.load(Ordering::SeqCst);

            Ok(tot)
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
                    })
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
                })
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
