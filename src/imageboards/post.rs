//! Main representation of a imageboard post
use super::rating::Rating;
use crate::{
    progress_bars::{download_progress_style, ProgressCounter},
    ImageBoards,
};
use ahash::AHashSet;
use anyhow::{bail, Error};
use colored::Colorize;
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressDrawTarget};
use log::debug;
use md5::compute;
use reqwest::Client;
use serde::Serialize;
use std::{
    cmp::Ordering,
    fs::File,
    io::Write,
    path::Path,
    sync::{Arc, Mutex},
};
use tokio::{
    fs::{self, read, OpenOptions},
    io::AsyncWriteExt,
    io::BufWriter,
};
use zip::{write::FileOptions, CompressionMethod, ZipWriter};

/// Generic representation of a imageboard post
/// Most imageboard APIs have a common set of info from the files we want to download.
/// This struct is just a catchall model for the necessary parts of the post the program needs to properly download and save the files.
#[derive(Debug, Clone, Serialize)]
pub struct Post {
    /// ID number of the post given by the imageboard
    pub id: u64,
    /// Direct URL of the original image file located inside the imageboard's server
    pub url: String,
    /// Instead of calculating the downloaded file's MD5 hash on the fly, it uses the one provided by the API.
    pub md5: String,
    /// The original file extension provided by the imageboard.
    ///
    /// ```https://konachan.com``` (Moebooru) and some other imageboards don't provide this field. So, additional work is required to get the file extension from the url
    pub extension: String,
    /// Rating of the post. Can be:
    ///
    /// * `Rating::Safe` for SFW posts
    /// * `Rating::Questionable` for a not necessarily SFW post
    /// * `Rating::Explicit` for NSFW posts
    /// * `Rating::Unknown` in case none of the above are correctly parsed
    pub rating: Rating,
    /// Set of tags associated with the post.
    ///
    /// Used to exclude posts according to a blacklist
    pub tags: AHashSet<String>,
}

impl Ord for Post {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for Post {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Post {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Post {}

impl Post {
    /// Main routine to download a single post.
    ///
    /// This function is normally part of a `DownloadQueue::download_post_list()` method.
    ///
    /// It can be used alone, but it's not advised.
    pub async fn get(
        &self,
        client: &Client,
        output: &Path,
        counters: Arc<ProgressCounter>,
        variant: ImageBoards,
        name_id: bool,
        zip: Option<Arc<Mutex<ZipWriter<File>>>>,
    ) -> Result<(), Error> {
        let name = if name_id {
            self.id.to_string()
        } else {
            self.md5.clone()
        };
        let output = output.join(format!("{}.{}", name, &self.extension));

        if Self::check_file_exists(self, &output, counters.clone(), name_id)
            .await
            .is_ok()
        {
            Self::fetch(self, client, counters, &output, variant, zip).await?;
        }
        Ok(())
    }

    async fn check_file_exists(
        &self,
        output: &Path,
        counters: Arc<ProgressCounter>,
        name_id: bool,
    ) -> Result<(), Error> {
        if output.exists() {
            let name = if name_id {
                self.id.to_string()
            } else {
                self.md5.clone()
            };
            let file_digest = compute(read(&output).await?);
            let hash = format!("{:x}", file_digest);
            if hash == self.md5 {
                counters.multi.println(format!(
                    "{} {} {}",
                    "File".bold().green(),
                    format!("{}.{}", &name, &self.extension).bold().green(),
                    "already exists. Skipping.".bold().green()
                ))?;
                counters.main.inc(1);
                *counters.total_mtx.lock().unwrap() += 1;
                bail!("")
            }

            fs::remove_file(&output).await?;
            counters.multi.println(format!(
                "{} {} {}",
                "File".bold().red(),
                format!("{}.{}", &name, &self.extension).bold().red(),
                "is corrupted. Re-downloading...".bold().red()
            ))?;

            Ok(())
        } else {
            Ok(())
        }
    }

    async fn fetch(
        &self,
        client: &Client,
        counters: Arc<ProgressCounter>,
        output: &Path,
        variant: ImageBoards,
        zip: Option<Arc<Mutex<ZipWriter<File>>>>,
    ) -> Result<(), Error> {
        debug!("Fetching {}", &self.url);
        let res = client.get(&self.url).send().await?;

        if res.status().is_client_error() {
            counters.multi.println(format!(
                "{} {}{}",
                "Image source returned status".bold().red(),
                res.status().as_str().bold().red(),
                ". Skipping download.".bold().red()
            ))?;
            counters.main.inc(1);
            bail!("Post is valid but original file doesn't exist")
        }

        let size = res.content_length().unwrap_or_default();
        let bar = ProgressBar::new(size)
            .with_style(download_progress_style(&variant.progress_template()));
        bar.set_draw_target(ProgressDrawTarget::stderr_with_hz(60));

        let pb = counters.multi.add(bar);

        debug!("Creating destination file {:?}", &output);

        // Download the file chunk by chunk.
        debug!("Retrieving chunks...");
        let mut stream = res.bytes_stream();

        if let Some(zf) = zip {
            let fvec: Vec<u8> = Vec::with_capacity(size as usize);

            let mut buf = BufWriter::with_capacity(size as usize, fvec);

            let options = FileOptions::default().compression_method(CompressionMethod::Stored);

            while let Some(item) = stream.next().await {
                // Retrieve chunk.
                let mut chunk = match item {
                    Ok(chunk) => chunk,
                    Err(e) => {
                        bail!(e)
                    }
                };
                pb.inc(chunk.len() as u64);

                // Write to file.
                buf.write_all_buf(&mut chunk).await?;

                // match file.write_all_buf(&mut chunk).await {
                //     Ok(_res) => (),
                //     Err(e) => {
                //         bail!(e);
                //     }
                // };
            }

            let mut un_mut = zf.lock().unwrap();
            un_mut.start_file(format!("{}.{}", self.md5, self.extension), options)?;

            un_mut.write_all(buf.buffer())?;
        } else {
            let mut file = OpenOptions::new()
                .append(true)
                .create(true)
                .open(output)
                .await?;

            while let Some(item) = stream.next().await {
                // Retrieve chunk.
                let mut chunk = match item {
                    Ok(chunk) => chunk,
                    Err(e) => {
                        bail!(e)
                    }
                };
                pb.inc(chunk.len() as u64);

                // Write to file.
                match file.write_all_buf(&mut chunk).await {
                    Ok(_res) => (),
                    Err(e) => {
                        bail!(e);
                    }
                };
            }
        }

        pb.finish_and_clear();

        counters.main.inc(1);
        let mut down_count = counters.downloaded_mtx.lock().unwrap();
        let mut total_count = counters.total_mtx.lock().unwrap();
        *total_count += 1;
        *down_count += 1;
        Ok(())
    }
}
