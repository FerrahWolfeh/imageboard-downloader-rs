//! Main representation of a imageboard post
//!
//! # Post
//! A [`Post` struct](Post) is a generic representation of an imageboard post.
//!
//! Most imageboard APIs have a common set of info from the files we want to download.
use crate::{progress_bars::ProgressCounter, ImageBoards};
use ahash::AHashSet;
use bytesize::ByteSize;
use colored::Colorize;
use futures::StreamExt;
use log::debug;
use md5::compute;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    convert::TryInto,
    fs::File,
    io::Write,
    ops::Not,
    path::Path,
    sync::{Arc, Mutex},
};
use tokio::{
    fs::{self, read, rename, OpenOptions},
    io::AsyncWriteExt,
    io::BufWriter,
    task::spawn_blocking,
};
use zip::{write::FileOptions, CompressionMethod, ZipWriter};

use self::{error::PostError, rating::Rating};

mod error;
pub mod rating;

#[derive(Debug, Clone, Copy)]
pub enum NameType {
    ID,
    MD5,
}

impl Not for NameType {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            NameType::ID => NameType::MD5,
            NameType::MD5 => NameType::ID,
        }
    }
}

impl PartialEq for NameType {
    fn eq(&self, other: &Self) -> bool {
        core::mem::discriminant(self) == core::mem::discriminant(other)
    }
}

/// Queue that combines all posts collected, with which tags and with a user-defined blacklist in case an Extractor implements [Auth](crate::imageboards::extractors::Auth).
#[derive(Debug)]
pub struct PostQueue {
    /// A list containing all `Post`s collected.
    pub posts: Vec<Post>,
    /// The tags used to search the collected posts.
    pub tags: Vec<String>,
}

/// Catchall model for the necessary parts of the imageboard post to properly identify, download and save it.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub async fn get(
        &self,
        client: &Client,
        output: &Path,
        counters: Arc<ProgressCounter>,
        variant: ImageBoards,
        name_type: NameType,
        zip: Option<Arc<Mutex<ZipWriter<File>>>>,
    ) -> Result<(), PostError> {
        let output = output.join(&self.file_name(name_type));

        if !self
            .check_file_exists(&output, counters.clone(), name_type)
            .await?
        {
            self.fetch(client, counters, &output, variant, zip).await?;
        }
        Ok(())
    }

    #[inline]
    pub fn file_name(&self, name_type: NameType) -> String {
        let name = match name_type {
            NameType::ID => self.id.to_string(),
            NameType::MD5 => self.md5.to_string(),
        };

        format!("{}.{}", name, self.extension)
    }

    async fn check_file_exists(
        &self,
        output: &Path,
        counters: Arc<ProgressCounter>,
        name_type: NameType,
    ) -> Result<bool, PostError> {
        let id_name = self.file_name(NameType::ID);
        let md5_name = self.file_name(NameType::MD5);

        let (name, inv_name) = match name_type {
            NameType::ID => (&id_name, &md5_name),
            NameType::MD5 => (&md5_name, &id_name),
        };

        let raw_path = output.parent().unwrap();

        let mut file_is_same = false;

        let actual = if output.exists() {
            debug!("File {} found.", &name);
            output.to_path_buf()
        } else if name_type == NameType::ID {
            debug!("File {} not found.", &name);
            debug!("Trying possibly matching file: {}", &md5_name);
            file_is_same = true;
            raw_path.join(Path::new(&md5_name))
        } else {
            debug!("File {} not found.", &name);
            debug!("Trying possibly matching file: {}", &id_name);
            file_is_same = true;
            raw_path.join(Path::new(&id_name))
        };

        if actual.exists() {
            debug!("Checking MD5 sum of {}.", inv_name);
            let file_digest = compute(read(&actual).await?);
            let hash = format!("{:x}", file_digest);
            if hash == self.md5 {
                debug!("MD5 matches. File is OK.");
                if file_is_same {
                    debug!("Found similar file in directory, renaming.");
                    match counters.multi.println(format!(
                        "{} {} {}",
                        "A file similar to".bold().green(),
                        name.bold().blue().italic(),
                        "already exists and will be renamed accordingly."
                            .bold()
                            .green()
                    )) {
                        Ok(_) => {
                            rename(&actual, output).await?;
                        }
                        Err(error) => {
                            return Err(PostError::ProgressBarPrintFail {
                                message: error.to_string(),
                            })
                        }
                    };

                    counters.main.inc(1);
                    *counters.total_mtx.lock().unwrap() += 1;
                    return Ok(true);
                }
                debug!("Skipping download.");
                match counters.multi.println(format!(
                    "{} {} {}",
                    "File".bold().green(),
                    name.bold().blue().italic(),
                    "already exists. Skipping.".bold().green()
                )) {
                    Ok(_) => (),
                    Err(error) => {
                        return Err(PostError::ProgressBarPrintFail {
                            message: error.to_string(),
                        })
                    }
                };

                counters.main.inc(1);
                *counters.total_mtx.lock().unwrap() += 1;
                return Ok(true);
            }

            debug!("MD5 doesn't match, File might be corrupted\nExpected: {}, got: {}\nRemoving file...", self.md5, hash);

            fs::remove_file(&output).await?;
            counters.multi.println(format!(
                "{} {} {}",
                "File".bold().red(),
                name.bold().yellow().italic(),
                "is corrupted. Re-downloading...".bold().red()
            ))?;

            Ok(false)
        } else {
            Ok(false)
        }
    }

    async fn fetch(
        &self,
        client: &Client,
        counters: Arc<ProgressCounter>,
        output: &Path,
        variant: ImageBoards,
        zip: Option<Arc<Mutex<ZipWriter<File>>>>,
    ) -> Result<(), PostError> {
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
            return Err(PostError::RemoteFileNotFound);
        }

        let size = res.content_length().unwrap_or_default();

        debug!("Remote file is {}", ByteSize::b(size).to_string_as(true));

        let pb = counters.add_download_bar(size, variant);

        // Download the file chunk by chunk.
        debug!("Retrieving chunks...");
        let mut stream = res.bytes_stream();

        let buf_size: usize = size.try_into()?;

        if let Some(zf) = zip {
            let fvec: Vec<u8> = Vec::with_capacity(buf_size);

            let mut buf = BufWriter::with_capacity(buf_size, fvec);

            let options = FileOptions::default().compression_method(CompressionMethod::Stored);

            let arr = Arc::new(self.clone());

            while let Some(item) = stream.next().await {
                // Retrieve chunk.
                let mut chunk = match item {
                    Ok(chunk) => chunk,
                    Err(e) => {
                        return Err(PostError::ChunkDownloadFail {
                            message: e.to_string(),
                        })
                    }
                };
                pb.inc(chunk.len() as u64);

                // Write to file.
                buf.write_all_buf(&mut chunk).await?;
            }

            let ite = arr.clone();

            let file_name = output.file_stem().unwrap().to_str().unwrap().to_string();

            spawn_blocking(move || -> Result<(), PostError> {
                let mut un_mut = zf.lock().unwrap();

                let data = ite;

                debug!("Writing {}.{} to cbz file", file_name, data.extension);
                un_mut.start_file(
                    format!(
                        "{}/{}.{}",
                        data.rating.to_string(),
                        file_name,
                        data.extension
                    ),
                    options,
                )?;

                un_mut.write_all(buf.buffer())?;
                Ok(())
            })
            .await??;
        } else {
            debug!("Creating {:?}", &output);
            let file = OpenOptions::new()
                .append(true)
                .create(true)
                .open(output)
                .await?;

            let mut bw = BufWriter::new(file);

            while let Some(item) = stream.next().await {
                // Retrieve chunk.
                let mut chunk = match item {
                    Ok(chunk) => chunk,
                    Err(e) => {
                        return Err(PostError::ChunkDownloadFail {
                            message: e.to_string(),
                        })
                    }
                };
                pb.inc(chunk.len() as u64);

                // Write to file.
                bw.write_all_buf(&mut chunk).await?;
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
