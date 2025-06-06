//! Handles the logic for downloading posts and packaging them into CBZ archives.
//!
//! This module is responsible for fetching posts, adding them to a `ZipWriter` instance,
//! managing concurrent downloads, and structuring the CBZ file.
use std::{
    fs::File,
    io::Write,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use futures::StreamExt;
use ibdl_common::{
    log::debug,
    post::{NameType, Post, error::PostError, rating::Rating},
    reqwest::Client,
    tokio::{
        io::AsyncWriteExt,
        task::{self, spawn_blocking},
    },
};
use tokio_stream::wrappers::UnboundedReceiverStream;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::{error::QueueError, progress::SharedProgressListener};

use super::Queue;

impl Queue {
    /// Fetches a single post for a pool download and adds it to the CBZ archive.
    ///
    /// Files in a pool CBZ are typically named sequentially (e.g., "000001.jpg").
    /// This function handles the download, progress reporting for the individual file,
    /// and writing the file data to the provided `ZipWriter`.
    ///
    /// # Arguments
    ///
    /// * `client`: The `reqwest::Client` to use for the download.
    /// * `post`: The `Post` object to download.
    /// * `zip`: An `Arc<Mutex<ZipWriter<File>>>` representing the CBZ archive being written to.
    ///            The actual writing is done within a `spawn_blocking` task.
    /// * `num_digits`: The number of digits to use for the sequential filename (e.g., 6 for "000001").
    /// * `progress_listener`: A `SharedProgressListener` for reporting download progress and logging events.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the post is successfully downloaded and added to the CBZ.
    /// * `Err(PostError)` if there's an issue with downloading, writing to the zip,
    ///   or if the remote server returns an error.
    ///
    /// # Panics
    /// This function can panic if `zip.lock()` fails, which indicates a poisoned mutex.
    pub(crate) async fn fetch_cbz_pool(
        client: Client,
        post: Post,
        zip: Arc<Mutex<ZipWriter<File>>>,
        num_digits: usize,
        progress_listener: SharedProgressListener,
    ) -> Result<(), PostError> {
        let filename = post.seq_file_name(num_digits);
        debug!("Fetching {}", &post.url);
        let res = client.get(&post.url).send().await?;

        // Use the already computed filename for logging if skipping
        if res.status().is_client_error() {
            debug!(
                "Image source for {} returned status {}. Skipping download.",
                post.url,
                res.status().as_str()
            );
            progress_listener.log_skip_message(
                &filename,
                &format!("skipped, server returned: {}", res.status().as_str()),
            );
            return Err(PostError::RemoteFileNotFound);
        }

        let size = res.content_length().unwrap_or_default();
        let dl_updater = progress_listener.add_download_task(filename.clone(), Some(size));
        let mut downloaded_bytes = 0;

        debug!("Retrieving chunks for {}", &filename);
        let mut stream = res.bytes_stream();

        let mut fvec: Vec<u8> = Vec::with_capacity(size.try_into().unwrap_or(0));

        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        while let Some(item) = stream.next().await {
            // Retrieve chunk.
            let chunk = match item {
                Ok(chunk) => chunk,
                Err(e) => {
                    dl_updater.finish();
                    return Err(PostError::ChunkDownloadFail {
                        message: e.to_string(),
                    });
                }
            };
            let chunk_len = chunk.len() as u64;
            downloaded_bytes += chunk_len;
            dl_updater.set_progress(downloaded_bytes);

            // Write to file.
            AsyncWriteExt::write_all(&mut fvec, &chunk).await?;
        }
        spawn_blocking(move || -> Result<(), PostError> {
            let mut un_mut = zip.lock().unwrap();

            debug!("Writing {} to cbz file", filename);
            if let Err(error) = un_mut.start_file(filename, options) {
                return Err(PostError::ZipFileWriteError {
                    message: error.to_string(),
                });
            }

            un_mut.write_all(&fvec)?;

            drop(un_mut);

            Ok(())
        })
        .await
        .map_err(|thread_error| PostError::ZipThreadStartError {
            msg: thread_error.to_string(), // Corrected from ZipFileWriteError to ZipThreadStartError if it's from spawn_blocking
        })??;

        dl_updater.finish();

        Ok(())
    }

    /// Fetches a single post for a non-pool (tag-based) download and adds it to the CBZ archive.
    ///
    /// Files are typically placed into subdirectories within the CBZ based on their `Rating`
    /// (e.g., "Safe/image.jpg"). If `annotate` is true, a corresponding ".txt" caption file
    /// is also created and added to the CBZ.
    ///
    /// # Arguments
    ///
    /// * `client`: The `reqwest::Client` to use for the download.
    /// * `name_type`: The `NameType` (ID or MD5) to use for the image filename and caption filename.
    /// * `post`: The `Post` object to download.
    /// * `annotate`: A boolean indicating whether to create and add a caption file for the post.
    /// * `zip`: An `Arc<Mutex<ZipWriter<File>>>` representing the CBZ archive.
    ///            Writing is done within a `spawn_blocking` task.
    /// * `progress_listener`: A `SharedProgressListener` for reporting progress and logging.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the post (and caption, if applicable) is successfully downloaded and added.
    /// * `Err(PostError)` for download, zip writing, or remote server errors.
    ///
    /// # Panics
    /// This function can panic if `zip.lock()` fails (poisoned mutex).
    pub(crate) async fn fetch_cbz(
        client: Client,
        name_type: NameType,
        post: Post,
        annotate: bool,
        zip: Arc<Mutex<ZipWriter<File>>>,
        progress_listener: SharedProgressListener,
    ) -> Result<(), PostError> {
        let filename = post.file_name(name_type);
        debug!("Fetching {}", &post.url);
        let res = client.get(&post.url).send().await?;

        // Use the already computed filename for logging if skipping
        if res.status().is_client_error() {
            debug!(
                "Image source for {} returned status {}. Skipping download.",
                post.url,
                res.status().as_str()
            );
            progress_listener.log_skip_message(
                &filename,
                &format!("skipped, server returned: {}", res.status().as_str()),
            );
            return Err(PostError::RemoteFileNotFound);
        }

        let size = res.content_length().unwrap_or_default();
        let dl_updater = progress_listener.add_download_task(filename.clone(), Some(size));
        let mut downloaded_bytes = 0;

        debug!("Retrieving chunks for {}", &filename);
        let mut stream = res.bytes_stream();

        let mut fvec: Vec<u8> = Vec::with_capacity(size.try_into().unwrap_or(0));

        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        let cap_options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .compression_level(Some(5));

        while let Some(item) = stream.next().await {
            // Retrieve chunk.
            let chunk = match item {
                Ok(chunk) => chunk,
                Err(e) => {
                    dl_updater.finish();
                    return Err(PostError::ChunkDownloadFail {
                        message: e.to_string(),
                    });
                }
            };
            let chunk_len = chunk.len() as u64;
            downloaded_bytes += chunk_len;
            dl_updater.set_progress(downloaded_bytes);

            // Write to file.
            AsyncWriteExt::write_all(&mut fvec, &chunk).await?;
        }
        spawn_blocking(move || -> Result<(), PostError> {
            let mut un_mut = zip.lock().unwrap();

            debug!("Writing {} to cbz file", filename);
            if let Err(error) = un_mut.start_file(format!("{}/{}", post.rating, filename), options)
            {
                drop(un_mut);
                return Err(PostError::ZipFileWriteError {
                    message: error.to_string(),
                });
            };

            un_mut.write_all(&fvec)?;

            if annotate {
                debug!("Writing caption for {} to cbz file", filename);
                if let Err(error) = un_mut.start_file(
                    format!("{}/{}.txt", post.rating, post.name(name_type)),
                    cap_options,
                ) {
                    drop(un_mut);

                    return Err(PostError::ZipFileWriteError {
                        message: error.to_string(),
                    });
                };

                let tag_list = Vec::from_iter(
                    post.tags
                        .iter()
                        .filter(|t| t.is_prompt_tag())
                        .map(|tag| tag.tag()),
                );

                let prompt = tag_list.join(", ");

                let f1 = prompt.replace('_', " ");

                un_mut.write_all(f1.as_bytes())?;

                drop(un_mut);
            }
            Ok(())
        })
        .await
        .map_err(|thread_error| PostError::ZipThreadStartError {
            msg: thread_error.to_string(),
        })??;

        dl_updater.finish();

        Ok(())
    }

    /// Writes the initial directory structure to the CBZ file for non-pool downloads.
    ///
    /// This creates directories named after each `Rating` variant (Safe, Questionable, Explicit, Unknown)
    /// at the root of the CBZ archive. This is typically called once before any posts are added.
    ///
    /// # Arguments
    ///
    /// * `zip`: An `Arc<Mutex<ZipWriter<File>>>` representing the CBZ archive.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the directories are successfully created.
    /// * `Err(QueueError)` if there's an error writing to the zip file (converted from `zip::result::ZipError`).
    pub(crate) fn write_zip_structure(
        &self,
        zip: Arc<Mutex<ZipWriter<File>>>,
    ) -> Result<(), QueueError> {
        {
            let opts = SimpleFileOptions::default();

            let mut z_1 = zip.lock().unwrap();
            z_1.add_directory(Rating::Safe.to_string(), opts)?;
            z_1.add_directory(Rating::Questionable.to_string(), opts)?;
            z_1.add_directory(Rating::Explicit.to_string(), opts)?;
            z_1.add_directory(Rating::Unknown.to_string(), opts)?;
        }

        Ok(())
    }

    /// Orchestrates the download of posts from a channel and packages them into a CBZ archive.
    ///
    /// This function initializes a `ZipWriter`, sets up the directory structure if it's not a pool download,
    /// and then processes a stream of `Post` objects. For each post, it spawns a task
    /// (either `fetch_cbz_pool` or `fetch_cbz`) to download and add the post to the archive.
    /// The number of concurrent download tasks is limited by `self.sim_downloads`.
    ///
    /// # Arguments
    ///
    /// * `path`: The `PathBuf` where the final CBZ file will be saved.
    /// * `channel`: An `UnboundedReceiverStream` that yields `Post` objects to be downloaded.
    /// * `is_pool`: A boolean indicating if the download is for a pool. This affects how posts are fetched
    ///              and whether the initial directory structure is created.
    /// * `progress_listener`: A `SharedProgressListener` for reporting overall progress (main bar ticks)
    ///                        and individual file download progress/events.
    /// * `downloaded_post_count`: An `Arc<AtomicU64>` to count successfully downloaded and archived posts.
    ///
    /// # Behavior
    /// - Creates the CBZ file at the specified `path`.
    /// - If `!is_pool`, calls `write_zip_structure`.
    /// - For each post from the `channel`, `progress_listener.main_tick()` is called, and a download task is spawned.
    /// - After all posts are processed, the `ZipWriter` is finalized.
    /// - `downloaded_post_count` is incremented for each post successfully added to the CBZ.
    /// - Errors during individual post fetching are logged but do not stop the processing of other posts.
    pub(crate) async fn cbz_path(
        &self,
        path: PathBuf,
        channel: UnboundedReceiverStream<Post>,
        is_pool: bool,
        progress_listener: SharedProgressListener,
        // This counter is incremented *after* a post is successfully downloaded and added to the CBZ.
        // The main progress bar is ticked *before* the download task is spawned,
        // indicating the post has been received from the extractor.
        downloaded_post_count: Arc<AtomicU64>,
    ) -> Result<(), QueueError> {
        debug!("Target file: {}", path.display());

        let file = File::create(&path)?;
        let zip = Arc::new(Mutex::new(ZipWriter::new(file)));

        if !is_pool {
            self.write_zip_structure(zip.clone())?;
        }

        channel
            .map(|post_to_download| {
                // Increment main progress bar as soon as a post is received from the extractor channel
                progress_listener.main_tick();

                // Clone Arcs and values
                let nt_clone = self.name_type;
                let client_clone = self.client.clone();
                let zip_clone = zip.clone();
                let progress_listener_clone = progress_listener.clone();
                let annotate_clone = self.annotate;

                task::spawn(async move {
                    if is_pool {
                        Self::fetch_cbz_pool(
                            client_clone,
                            post_to_download,
                            zip_clone,
                            6, // num_digits for pool
                            progress_listener_clone,
                        )
                        .await
                    } else {
                        Self::fetch_cbz(
                            client_clone,
                            nt_clone,
                            post_to_download,
                            annotate_clone,
                            zip_clone,
                            progress_listener_clone,
                        )
                        .await
                    }
                })
            })
            .buffer_unordered(self.sim_downloads.into())
            .for_each(
                |task_join_result: Result<Result<(), PostError>, task::JoinError>| {
                    let downloaded_post_count_clone = downloaded_post_count.clone();
                    async move {
                        match task_join_result {
                            Ok(Ok(())) => {
                                // Successfully joined, and fetch was Ok
                                downloaded_post_count_clone.fetch_add(1, Ordering::SeqCst);
                            }
                            Ok(Err(post_error)) => {
                                // Successfully joined, but fetch failed
                                debug!("Failed to download and add post to CBZ: {}", post_error);
                            }
                            Err(join_error) => {
                                // Task panicked or was cancelled
                                debug!("CBZ post processing task failed: {}", join_error);
                            }
                        }
                    }
                },
            )
            .await;

        // Finalize the zip archive.
        // To call `finish()`, which consumes `self`, we need to obtain ownership
        // of the `ZipWriter`.
        // 1. Try to unwrap the `Arc` to get the `Mutex`. This succeeds if this is the
        //    last `Arc` pointer to the `Mutex`. All tasks using clones of the `Arc`
        //    must have completed and dropped their clones.
        // 2. Get the `ZipWriter` from the `Mutex` using `into_inner()`. This consumes
        //    the `Mutex` and returns the inner data.
        let zip_writer_mutex = Arc::try_unwrap(zip)
            .map_err(|_arc_still_has_clones| QueueError::MutexLockReleaseError)?;
        let zip_writer = zip_writer_mutex
            .into_inner()
            .map_err(|_poison_error| QueueError::MutexLockReleaseError)?;

        zip_writer.finish()?; // Consumes `zip_writer` and finalizes the archive.
        Ok(())
    }
}
