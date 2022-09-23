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
use futures::StreamExt;
use ibdl_common::colored::Colorize;
use ibdl_common::log::debug;
use ibdl_common::post::error::PostError;
use ibdl_common::post::rating::Rating;
use ibdl_common::post::{NameType, Post, PostQueue};
use ibdl_common::reqwest::Client;
use ibdl_common::{client, tokio, ImageBoards};
use md5::compute;
use std::convert::TryInto;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use tokio::fs::{create_dir_all, read, remove_file, rename, OpenOptions};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::task::{self, spawn_blocking};
use zip::write::FileOptions;
use zip::CompressionMethod;
use zip::ZipWriter;

use crate::progress_bars::ProgressCounter;

use self::error::QueueError;
use self::summary::{SummaryFile, SummaryType};

mod error;
pub mod summary;

macro_rules! finish_and_increment {
    ($x:expr) => {{
        $x.main.inc(1);
        let mut down_count = $x.downloaded_mtx.lock().unwrap();
        let mut total_count = $x.total_mtx.lock().unwrap();
        *total_count += 1;
        *down_count += 1;
    }};
}

/// Struct where all the downloading and filtering will take place
pub struct Queue {
    list: Vec<Post>,
    tags: Vec<String>,
    imageboard: ImageBoards,
    sim_downloads: u8,
    client: Client,
    cbz: bool,
}

impl Queue {
    /// Set up the queue for download
    #[must_use]
    pub fn new(
        imageboard: ImageBoards,
        posts: PostQueue,
        sim_downloads: u8,
        custom_client: Option<Client>,
        save_as_cbz: bool,
    ) -> Self {
        let client = if let Some(cli) = custom_client {
            cli
        } else {
            client!(imageboard)
        };

        Self {
            list: posts.posts,
            tags: posts.tags,
            cbz: save_as_cbz,
            imageboard,
            sim_downloads,
            client,
        }
    }

    async fn create_out(&self, dir: PathBuf) -> Result<PathBuf, QueueError> {
        if self.cbz {
            let output_file = dir.parent().unwrap().to_path_buf();

            match create_dir_all(&output_file).await {
                Ok(_) => (),
                Err(error) => {
                    return Err(QueueError::DirCreationError {
                        message: error.to_string(),
                    })
                }
            };
            return Ok(output_file);
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

        Ok(dir)
    }

    /// Starts the download of all posts collected inside a [`PostQueue`]
    pub async fn download(
        self,
        output_dir: PathBuf,
        name_type: NameType,
    ) -> Result<u64, QueueError> {
        let counters = ProgressCounter::initialize(self.list.len().try_into()?, self.imageboard);

        let output_place = self.create_out(output_dir).await?;

        if self.cbz {
            self.cbz_path(output_place, counters.clone(), name_type)
                .await?;

            counters.main.finish_and_clear();

            return Ok(*counters.downloaded_mtx.lock().unwrap());
        }

        debug!("Fetching {} posts", self.list.len());

        futures::stream::iter(self.list)
            .map(|d| {
                let cli = self.client.clone();
                let output = output_place.join(d.file_name(name_type));
                let variant = self.imageboard;
                let counters = counters.clone();

                task::spawn(async move {
                    if !Self::check_file_exists(&d, &output, counters.clone(), name_type).await? {
                        Self::fetch(cli, variant, d, counters, &output).await?;
                    }
                    Ok::<(), QueueError>(())
                })
            })
            .buffer_unordered(self.sim_downloads as usize)
            .for_each(|_| async {})
            .await;

        counters.main.finish_and_clear();

        let tot = *counters.downloaded_mtx.lock().unwrap();

        Ok(tot)
    }

    async fn cbz_path(
        &self,
        path: PathBuf,
        counters: Arc<ProgressCounter>,
        name_type: NameType,
    ) -> Result<(), QueueError> {
        debug!("Target file: {}", path.display());

        let file = File::create(&path)?;
        let zip = Arc::new(Mutex::new(ZipWriter::new(file)));

        self.write_zip_structure(zip.clone(), &self.list.clone(), name_type)?;

        debug!("Fetching {} posts", self.list.len());

        futures::stream::iter(self.list.clone())
            .map(|d| {
                let cli = self.client.clone();
                let variant = self.imageboard;
                let counters = counters.clone();
                let zip = zip.clone();

                task::spawn(async move {
                    Self::fetch_cbz(cli, variant, name_type, d, counters, zip).await?;
                    Ok::<(), QueueError>(())
                })
            })
            .buffer_unordered(self.sim_downloads.into())
            .for_each(|_| async {})
            .await;

        let mut mtx = zip.lock().unwrap();

        mtx.finish()?;
        Ok(())
    }

    fn write_zip_structure(
        &self,
        zip: Arc<Mutex<ZipWriter<File>>>,
        posts: &[Post],
        name_type: NameType,
    ) -> Result<(), QueueError> {
        let ap = SummaryFile::new(
            self.imageboard,
            &self.tags,
            posts,
            name_type,
            SummaryType::JSON,
        )
        .to_json()?;

        let mut z_1 = zip.lock().unwrap();

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
        Ok(())
    }

    async fn check_file_exists(
        post: &Post,
        output: &Path,
        counters: Arc<ProgressCounter>,
        name_type: NameType,
    ) -> Result<bool, QueueError> {
        let id_name = post.file_name(NameType::ID);
        let md5_name = post.file_name(NameType::MD5);

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
            if hash == post.md5 {
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
                            return Err(QueueError::ProgressBarPrintFail {
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
                        return Err(QueueError::ProgressBarPrintFail {
                            message: error.to_string(),
                        })
                    }
                };

                counters.main.inc(1);
                *counters.total_mtx.lock().unwrap() += 1;
                return Ok(true);
            }

            debug!("MD5 doesn't match, File might be corrupted\nExpected: {}, got: {}\nRemoving file...", post.md5, hash);

            remove_file(&actual).await?;
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

    async fn fetch_cbz(
        client: Client,
        variant: ImageBoards,
        name_type: NameType,
        post: Post,
        counters: Arc<ProgressCounter>,
        zip: Arc<Mutex<ZipWriter<File>>>,
    ) -> Result<(), PostError> {
        let filename = post.file_name(name_type);
        debug!("Fetching {}", &post.url);
        let res = client.get(&post.url).send().await?;

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

        let pb = counters.add_download_bar(size, variant);

        // Download the file chunk by chunk.
        debug!("Retrieving chunks for {}", &filename);
        let mut stream = res.bytes_stream();

        let buf_size: usize = size.try_into()?;

        let mut fvec: Vec<u8> = Vec::with_capacity(buf_size);

        let options = FileOptions::default().compression_method(CompressionMethod::Stored);

        while let Some(item) = stream.next().await {
            // Retrieve chunk.
            let chunk = match item {
                Ok(chunk) => chunk,
                Err(e) => {
                    return Err(PostError::ChunkDownloadFail {
                        message: e.to_string(),
                    })
                }
            };
            pb.inc(chunk.len().try_into()?);

            // Write to file.
            AsyncWriteExt::write_all(&mut fvec, &chunk).await?;
        }

        spawn_blocking(move || -> Result<(), PostError> {
            let mut un_mut = zip.lock().unwrap();

            debug!("Writing {} to cbz file", filename);
            match un_mut.start_file(format!("{}/{}", post.rating.to_string(), filename), options) {
                Ok(_) => {}
                Err(error) => {
                    return Err(PostError::ZipFileWriteError {
                        message: error.to_string(),
                    })
                }
            };

            un_mut.write_all(&fvec)?;
            Ok(())
        })
        .await??;

        pb.finish_and_clear();

        finish_and_increment!(counters);

        Ok(())
    }

    async fn fetch(
        client: Client,
        variant: ImageBoards,
        post: Post,
        counters: Arc<ProgressCounter>,
        output: &Path,
    ) -> Result<(), PostError> {
        debug!("Fetching {}", &post.url);
        let res = client.get(&post.url).send().await?;

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

        let pb = counters.add_download_bar(size, variant);

        // Download the file chunk by chunk.
        debug!("Retrieving chunks...");
        let mut stream = res.bytes_stream();

        let buf_size: usize = size.try_into()?;

        debug!("Creating {:?}", &output);
        let file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(output)
            .await?;

        let mut bw = BufWriter::with_capacity(buf_size, file);

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
            pb.inc(chunk.len().try_into()?);

            // Write to file.
            bw.write_all_buf(&mut chunk).await?;
        }
        bw.flush().await?;

        pb.finish_and_clear();

        finish_and_increment!(counters);

        Ok(())
    }
}
