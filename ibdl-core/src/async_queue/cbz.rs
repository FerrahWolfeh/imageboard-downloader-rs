use std::{fs::File, io::Write, path::PathBuf, sync::Arc, sync::Mutex};

use futures::StreamExt;
use ibdl_common::{
    log::debug,
    post::{error::PostError, rating::Rating, NameType, Post},
    reqwest::Client,
    tokio::{
        io::AsyncWriteExt,
        sync::mpsc::Sender,
        task::{self, spawn_blocking},
    },
    ImageBoards,
};
use owo_colors::OwoColorize;
use tokio_stream::wrappers::UnboundedReceiverStream;
use zip::{write::FileOptions, CompressionMethod, ZipWriter};

use crate::{async_queue::get_counters, error::QueueError};

use super::Queue;

impl Queue {
    pub(crate) async fn fetch_cbz_pool(
        client: Client,
        variant: ImageBoards,
        post: Post,
        zip: Arc<Mutex<ZipWriter<File>>>,
        num_digits: usize,
    ) -> Result<(), PostError> {
        let counters = get_counters();

        let filename = post.seq_file_name(num_digits);
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
            match un_mut.start_file(filename, options) {
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

        Ok(())
    }

    pub(crate) async fn fetch_cbz(
        client: Client,
        variant: ImageBoards,
        name_type: NameType,
        post: Post,
        annotate: bool,
        zip: Arc<Mutex<ZipWriter<File>>>,
    ) -> Result<(), PostError> {
        let counters = get_counters();
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
        let cap_options = FileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .compression_level(Some(5));

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

            if annotate {
                debug!("Writing caption for {} to cbz file", filename);
                match un_mut.start_file(
                    format!("{}/{}.txt", post.rating.to_string(), post.name(name_type)),
                    cap_options,
                ) {
                    Ok(_) => {}
                    Err(error) => {
                        return Err(PostError::ZipFileWriteError {
                            message: error.to_string(),
                        })
                    }
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
            }
            Ok(())
        })
        .await??;

        pb.finish_and_clear();

        Ok(())
    }

    pub(crate) fn write_zip_structure(
        &self,
        zip: Arc<Mutex<ZipWriter<File>>>,
    ) -> Result<(), QueueError> {
        let mut z_1 = zip.lock().unwrap();
        z_1.add_directory(Rating::Safe.to_string(), FileOptions::default())?;
        z_1.add_directory(Rating::Questionable.to_string(), FileOptions::default())?;
        z_1.add_directory(Rating::Explicit.to_string(), FileOptions::default())?;
        z_1.add_directory(Rating::Unknown.to_string(), FileOptions::default())?;

        Ok(())
    }

    pub(crate) async fn cbz_path(
        &self,
        path: PathBuf,
        progress_channel: Sender<bool>,
        channel: UnboundedReceiverStream<Post>,
        pool: bool,
    ) -> Result<(), QueueError> {
        debug!("Target file: {}", path.display());

        let file = File::create(&path)?;
        let zip = Arc::new(Mutex::new(ZipWriter::new(file)));

        if !pool {
            self.write_zip_structure(zip.clone())?;
        }
        let sender = progress_channel.clone();

        channel
            .map(|d| {
                let nt = self.name_type;

                let cli = self.client.clone();
                let zip = zip.clone();
                let variant = self.imageboard;
                let annotate = self.annotate;
                let sender = sender.clone();

                task::spawn(async move {
                    if pool {
                        Self::fetch_cbz_pool(cli, variant, d, zip, 6).await?;
                    } else {
                        Self::fetch_cbz(cli, variant, nt, d, annotate, zip).await?;
                    }

                    let _ = sender.send(true).await;
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
}
