//! Common functions for all imageboard downloader modules.
use crate::imageboards::auth::ImageboardConfig;
use crate::imageboards::ImageBoards;
use crate::progress_bars::download_progress_style;
use anyhow::{bail, Error};
use colored::Colorize;
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget};
use log::debug;
use md5::compute;
use reqwest::Client;
use std::collections::HashSet;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::fs;
use tokio::fs::{read, OpenOptions};
use tokio::io::AsyncWriteExt;

/// Checks if ```output_dir``` is set in cli args then returns a ```PathBuf``` pointing to where the files will be downloaded.
///
/// In case the user does not set a value with the ```-o``` flag, the function will default to the current dir where the program is running.
///
/// The path chosen will always end with the imageboard name followed by the tags used.
///
/// ```rust
///
/// use std::path::PathBuf;
/// use imageboard_downloader::imageboards::ImageBoards;
/// use imageboard_downloader::join_tags;
///
/// let tags = join_tags!(["kroos_(arknights)", "weapon"]);
/// let path = Some(PathBuf::from("./"));
///
/// let out_dir = generate_out_dir(path, &tags, ImageBoards::Danbooru).unwrap();
///
/// assert_eq!(PathBuf::from("./danbooru/kroos_(arknights)+weapon"), out_dir);
/// ```
pub fn generate_out_dir(
    out_dir: Option<PathBuf>,
    tags: &[String],
    imageboard: ImageBoards,
) -> Result<PathBuf, Error> {
    // If out_dir is not set via cli flags, use current dir
    let place = match out_dir {
        None => std::env::current_dir()?,
        Some(dir) => dir,
    };
    let tags = tags.join(" ");

    let out = place.join(PathBuf::from(format!(
        "{}/{}",
        imageboard.to_string(),
        tags
    )));
    debug!("Target dir: {}", out.display());
    Ok(out)
}

/// Struct to condense a commonly used duo of progress bar instances.
///
/// The main usage for this is to pass references of the progress bars across multiple threads while downloading.
pub struct ProgressArcs {
    pub main: Arc<ProgressBar>,
    pub multi: Arc<MultiProgress>,
}

/// Struct to condense both counters that are used when downloading and checking limits
#[derive(Clone)]
pub struct Counters {
    pub total_mtx: Arc<Mutex<usize>>,
    pub downloaded_mtx: Arc<Mutex<u64>>,
}

/// Generic representation of a imageboard post
/// Most imageboard APIs have a common set of info from the files we want to download.
/// This struct is just a catchall model for the necessary parts of the post the program needs to properly download and save the files.
#[derive(Debug, Clone)]
pub struct Post {
    pub id: u64,
    /// Direct URL of the original image file located inside the imageboard's server
    pub url: String,
    /// Instead of calculating the downloaded file's MD5 hash on the fly, it uses the one provided by the API and serves as the name of the downloaded file.
    pub md5: String,
    /// The original file extension provided by the imageboard.
    ///
    /// ```https://konachan.com``` and some other imageboards don't provide this field. So, additional work is required to get the file extension from the url
    pub extension: String,
    /// Set of tags associated with the post.
    ///
    /// Used to exclude posts according to a blacklist
    pub tags: HashSet<String>,
}

pub struct DownloadQueue {
    pub list: Vec<Post>,
    pub concurrent_downloads: usize,
    pub counters: Arc<Counters>,
}

impl DownloadQueue {
    pub fn new(
        list: Vec<Post>,
        concurrent_downloads: usize,
        limit: Option<usize>,
        counters: Counters,
    ) -> Self {
        let list = if let Some(max) = limit {
            let dt = *counters.total_mtx.lock().unwrap();
            let l_len = list.len();
            let ran = max - dt;
            if ran >= l_len {
                list
            } else {
                list[0..ran].to_vec()
            }
        } else {
            list
        };

        Self {
            list,
            concurrent_downloads,
            counters: Arc::new(counters),
        }
    }

    pub async fn download_post_list(
        self,
        client: &Client,
        output_dir: &Path,
        bars: Arc<ProgressArcs>,
        variant: ImageBoards,
        save_as_id: bool,
    ) -> Result<(), Error> {
        futures::stream::iter(&self.list)
            .map(|d| {
                d.get(
                    client,
                    output_dir,
                    bars.clone(),
                    variant,
                    self.counters.clone(),
                    save_as_id,
                )
            })
            .buffer_unordered(self.concurrent_downloads)
            .collect::<Vec<_>>()
            .await;

        Ok(())
    }
}

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
        bars: Arc<ProgressArcs>,
        variant: ImageBoards,
        counters: Arc<Counters>,
        name_id: bool,
    ) -> Result<(), Error> {
        let name = if name_id {
            self.id.to_string()
        } else {
            self.md5.clone()
        };
        let output = output.join(format!("{}.{}", name, &self.extension));

        if Self::check_file_exists(
            self,
            &output,
            bars.multi.clone(),
            bars.main.clone(),
            name_id,
            counters.total_mtx.clone(),
        )
        .await
        .is_ok()
        {
            Self::fetch(
                self,
                client,
                bars,
                &output,
                variant,
                counters.clone(),
            )
                .await?;
        }
        Ok(())
    }

    async fn check_file_exists(
        &self,
        output: &Path,
        multi_progress: Arc<MultiProgress>,
        main_bar: Arc<ProgressBar>,
        name_id: bool,
        total_ct_mtx: Arc<Mutex<usize>>,
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
                multi_progress.println(format!(
                    "{} {} {}",
                    "File".bold().green(),
                    format!("{}.{}", &name, &self.extension).bold().green(),
                    "already exists. Skipping.".bold().green()
                ))?;
                main_bar.inc(1);
                *total_ct_mtx.lock().unwrap() += 1;
                bail!("")
            }

            fs::remove_file(&output).await?;
            multi_progress.println(format!(
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
        bars: Arc<ProgressArcs>,
        output: &Path,
        variant: ImageBoards,
        counters: Arc<Counters>,
    ) -> Result<(), Error> {
        debug!("Fetching {}", &self.url);
        let res = client.get(&self.url).send().await?;

        if res.status().is_client_error() {
            bars.multi.println(format!(
                "{} {}{}",
                "Image source returned status".bold().red(),
                res.status().as_str().bold().red(),
                ". Skipping download.".bold().red()
            ))?;
            bars.main.inc(1);
            bail!("Post is valid but original file doesn't exist")
        }

        let size = res.content_length().unwrap_or_default();
        let bar = ProgressBar::new(size)
            .with_style(download_progress_style(&variant.progress_template()));
        bar.set_draw_target(ProgressDrawTarget::stderr_with_hz(60));

        let pb = bars.multi.add(bar);

        debug!("Creating destination file {:?}", &output);
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(output)
            .await?;

        // Download the file chunk by chunk.
        debug!("Retrieving chunks...");
        let mut stream = res.bytes_stream();
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
        pb.finish_and_clear();

        bars.main.inc(1);
        let mut down_count = counters.downloaded_mtx.lock().unwrap();
        let mut total_count = counters.total_mtx.lock().unwrap();
        *total_count += 1;
        *down_count += 1;
        Ok(())
    }
}

pub async fn try_auth(
    auth_state: bool,
    imageboard: ImageBoards,
    client: &Client,
) -> Result<(), Error> {
    if auth_state {
        let mut username = String::new();
        let mut api_key = String::new();
        let stdin = io::stdin();
        println!(
            "{} {}",
            "Logging into:".bold(),
            imageboard.to_string().green().bold()
        );
        print!("{}", "Username: ".bold());
        io::stdout().flush()?;
        stdin.read_line(&mut username)?;
        print!("{}", "API Key: ".bold());
        io::stdout().flush()?;
        stdin.read_line(&mut api_key)?;

        debug!("Username: {:?}", username.trim());
        debug!("API key: {:?}", api_key.trim());

        let mut at = ImageboardConfig::new(
            imageboard,
            username.trim().to_string(),
            api_key.trim().to_string(),
        );

        at.authenticate(client).await?;

        return Ok(());
    }
    Ok(())
}
