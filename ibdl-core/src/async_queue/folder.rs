use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;

use futures::StreamExt;
use ibdl_common::post::error::PostError;
use ibdl_common::{
    log::debug,
    post::{NameType, Post},
    reqwest::Client,
    tokio::{
        fs::{read, remove_file, rename, OpenOptions},
        io::{AsyncWriteExt, BufWriter},
        task,
    },
};
use md5::compute;
use tokio_stream::wrappers::UnboundedReceiverStream;

use std::sync::{atomic::Ordering, Arc};

// Using PostError for file operation errors within this module
// use crate::error::QueueError;
use crate::progress::SharedProgressListener;

use super::Queue;

/// Represents the outcome of a download attempt for a single post to a folder.
#[derive(Debug)]
enum FolderDownloadTaskStatus {
    Downloaded(Post), // Post was successfully downloaded
    Skipped(Post),    // Post was skipped (e.g., already exists or renamed)
}

impl Queue {
    pub(crate) async fn download_channel(
        &self,
        channel: UnboundedReceiverStream<Post>,
        output_dir: PathBuf,
        is_pool: bool,
        progress_listener: SharedProgressListener,
        downloaded_post_count: Arc<AtomicU64>,
    ) {
        channel
            .map(|post_to_download| {
                let nt_clone = self.name_type;
                let client_clone = self.client.clone();
                let output_dir_clone = output_dir.clone();
                let progress_listener_clone = progress_listener.clone();

                // Increment main progress bar as soon as a post is received from the extractor channel
                progress_listener.main_tick();

                task::spawn(async move {
                    let target_file_path =
                        output_dir_clone.join(post_to_download.file_name(nt_clone));

                    match Self::check_file_exists(
                        &post_to_download,
                        &target_file_path,
                        nt_clone,
                        &progress_listener_clone,
                    )
                    .await
                    {
                        Ok(true) => {
                            // File exists and is identical, or was renamed. Skip download.
                            // Message already logged by check_file_exists.
                            Ok(FolderDownloadTaskStatus::Skipped(post_to_download))
                        }
                        Ok(false) => {
                            // File does not exist or was removed due to MD5 mismatch. Proceed to fetch.
                            Self::fetch(
                                client_clone,
                                &post_to_download,
                                &output_dir_clone, // fetch will join the filename
                                nt_clone,
                                is_pool,
                                progress_listener_clone,
                            )
                            .await?; // Propagates PostError if fetch fails
                            Ok(FolderDownloadTaskStatus::Downloaded(post_to_download))
                        }
                        Err(e) => Err(e), // Propagate PostError from check_file_exists
                    }
                })
            })
            .buffer_unordered(self.sim_downloads as usize)
            .for_each(
                |task_join_result: Result<
                    Result<FolderDownloadTaskStatus, PostError>,
                    task::JoinError,
                >| {
                    let downloaded_post_count_clone = downloaded_post_count.clone();
                    let output_dir_clone = output_dir.clone();
                    let name_type_clone = self.name_type;
                    let annotate_clone = self.annotate;

                    async move {
                        match task_join_result {
                            Ok(Ok(task_status)) => {
                                // Task joined and completed successfully
                                match task_status {
                                    FolderDownloadTaskStatus::Downloaded(downloaded_post) => {
                                        if annotate_clone {
                                            if let Err(error) = Self::write_caption(
                                                &downloaded_post,
                                                name_type_clone,
                                                &output_dir_clone,
                                            )
                                            .await
                                            {
                                                debug!(
                                                    "{} {}: {}",
                                                    "Failed to write caption file for",
                                                    downloaded_post.file_name(name_type_clone),
                                                    error
                                                );
                                            }
                                        }
                                        downloaded_post_count_clone.fetch_add(1, Ordering::SeqCst);
                                    }
                                    FolderDownloadTaskStatus::Skipped(skipped_post) => {
                                        // Message already logged by check_file_exists.
                                        // No increment to downloaded_post_count.
                                        debug!(
                                            "Post {} (file: {}) was skipped.",
                                            skipped_post.id,
                                            skipped_post.file_name(name_type_clone)
                                        );
                                    }
                                }
                            }
                            Ok(Err(post_error)) => {
                                // Task joined, but the operation inside (check_file_exists or fetch) failed
                                debug!("Failed to process post: {}", post_error);
                            }
                            Err(join_error) => {
                                // Task panicked or was cancelled
                                debug!("Download task failed to execute: {}", join_error);
                            }
                        }
                    }
                },
            )
            .await;
    }

    /// Checks if the file exists, possibly with a different naming scheme (ID vs MD5).
    /// If a similar file exists (same MD5, different name scheme), it's renamed.
    /// If an identical file exists (same MD5, same name scheme), it's skipped.
    /// If a file with the same name exists but different MD5, it's removed.
    /// Logs actions using the progress_listener.
    ///
    /// # Arguments
    /// * `post`: The post object.
    /// * `target_path_full`: The full path where the file *should* be saved with the *target* naming convention.
    /// * `name_type`: The target naming convention (ID or MD5).
    /// * `progress_listener`: For logging skip/rename messages.
    ///
    /// # Returns
    /// * `Ok(true)`: File exists and is identical (or was renamed). Download should be **skipped**.
    /// * `Ok(false)`: File does not exist or was removed (MD5 mismatch). Download should **proceed**.
    /// * `Err(PostError)`: For file I/O errors during the check.
    async fn check_file_exists(
        post: &Post,
        target_path_full: &Path, // e.g., /output/dir/md5_name.ext or /output/dir/id_name.ext
        name_type: NameType,
        progress_listener: &SharedProgressListener,
    ) -> Result<bool, PostError> {
        let target_file_name = post.file_name(name_type); // The name it *should* have

        let output_dir = target_path_full.parent().unwrap();

        // Determine the alternative name and path
        let (alternative_name, alternative_path) = match name_type {
            NameType::ID => {
                let alt_name = post.file_name(NameType::MD5);
                (alt_name.clone(), output_dir.join(alt_name))
            }
            NameType::MD5 => {
                let alt_name = post.file_name(NameType::ID);
                (alt_name.clone(), output_dir.join(alt_name))
            }
        };

        // First, check if the file exists with the target name
        if target_path_full.exists() {
            let file_content = read(target_path_full).await?;
            let file_digest = compute(file_content);
            let hash = format!("{:x}", file_digest);

            if hash == post.md5 {
                progress_listener.log_skip_message(
                    &target_file_name,
                    "already exists and is identical (MD5 match)",
                );
                return Ok(true); // Identical file exists, skip download
            }
            // MD5 mismatch for the target file name
            remove_file(target_path_full).await?;
            progress_listener.log_skip_message(
                &target_file_name,
                "removed existing file (MD5 mismatch), will redownload",
            );
            return Ok(false); // Removed, proceed with download
        }

        // If target name doesn't exist, check the alternative name
        if alternative_path.exists() {
            let file_content = read(&alternative_path).await?;
            let file_digest = compute(file_content);
            let hash = format!("{:x}", file_digest);

            if hash == post.md5 {
                // MD5 matches, but name is alternative. Rename it.
                rename(&alternative_path, target_path_full).await?;
                progress_listener.log_skip_message(
                    &target_file_name,
                    &format!("renamed from {} (MD5 match)", alternative_name),
                );
                return Ok(true); // Renamed successfully, skip download
            }
            // MD5 mismatch for the alternative file name
            remove_file(&alternative_path).await?;
            progress_listener.log_skip_message(
                &target_file_name, // Log against the name we *intended* to write
                &format!(
                    "removed existing file {} (MD5 mismatch), will redownload",
                    alternative_name
                ),
            );
            return Ok(false); // Removed, proceed with download
        }

        // Neither target nor alternative name exists
        Ok(false) // Proceed with download
    }

    async fn fetch(
        client: Client,
        post: &Post,
        output_dir: &Path, // Directory where the file will be saved
        name_type: NameType,
        is_pool: bool,
        progress_listener: SharedProgressListener,
    ) -> Result<(), PostError> {
        let fname = if is_pool {
            post.seq_file_name(6)
        } else {
            post.file_name(name_type)
        };

        debug!(
            "Fetching {} for post ID {} into file {}",
            &post.url, post.id, fname
        );

        let res = client.get(&post.url).send().await?;

        if res.status().is_client_error() {
            debug!(
                "Image source for {} (file: {}) returned status {}. Skipping download.",
                post.url,
                fname,
                res.status().as_str()
            );
            progress_listener.log_skip_message(
                &fname,
                &format!("skipped, server returned: {}", res.status()),
            );
            return Err(PostError::RemoteFileNotFound);
        }

        let size = res.content_length().unwrap_or_default();
        let dl_updater = progress_listener.add_download_task(fname.clone(), Some(size));
        let mut downloaded_bytes = 0;

        let mut stream = res.bytes_stream();
        let out_path = output_dir.join(&fname);

        debug!("Creating/writing to file {:?}", &out_path);
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&out_path) // Use out_path here
            .await?;

        let mut bw = BufWriter::new(file);

        while let Some(item) = stream.next().await {
            let mut chunk = match item {
                Ok(chunk) => chunk,
                Err(e) => {
                    dl_updater.finish(); // Ensure updater is finished on error
                    return Err(PostError::ChunkDownloadFail {
                        message: e.to_string(),
                    });
                }
            };
            let chunk_len = chunk.len() as u64;
            downloaded_bytes += chunk_len;
            dl_updater.set_progress(downloaded_bytes);

            if let Err(e) = bw.write_all_buf(&mut chunk).await {
                dl_updater.finish();
                return Err(e.into()); // Converts std::io::Error to PostError via From impl
            }
        }

        if let Err(e) = bw.flush().await {
            dl_updater.finish();
            return Err(e.into());
        }

        dl_updater.finish();
        debug!("Finished downloading {} successfully.", fname);
        Ok(())
    }
}
