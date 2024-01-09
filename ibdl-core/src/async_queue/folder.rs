use std::path::{Path, PathBuf};

use futures::StreamExt;
use ibdl_common::{
    log::debug,
    post::{error::PostError, NameType, Post},
    reqwest::Client,
    tokio::{
        fs::{read, remove_file, rename, OpenOptions},
        io::{AsyncWriteExt, BufWriter},
        sync::mpsc::Sender,
        task,
    },
    ImageBoards,
};
use md5::compute;
use owo_colors::OwoColorize;
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::error::QueueError;

use super::{get_counters, Queue};

impl Queue {
    pub(crate) async fn download_channel(
        &self,
        channel: UnboundedReceiverStream<Post>,
        progress: Sender<bool>,
        output_dir: PathBuf,
        pool: bool,
    ) {
        let sender = progress.clone();

        channel
            .map(|d| {
                let nt = self.name_type;

                let cli = self.client.clone();
                let output = output_dir.clone();
                let file_path = output_dir.join(d.file_name(self.name_type));
                let variant = self.imageboard.server;
                let sender_chn = sender.clone();

                task::spawn(async move {
                    if !Self::check_file_exists(&d, &file_path, nt).await? {
                        Self::fetch(cli, variant, &d, &output, nt, pool).await?;
                    }
                    let _ = sender_chn.send(true).await;

                    Ok::<Post, QueueError>(d)
                })
            })
            .buffer_unordered(self.sim_downloads as usize)
            .for_each(|task| async {
                if let Ok(Ok(post)) = task {
                    if self.annotate {
                        if let Err(error) =
                            Self::write_caption(&post, self.name_type, &output_dir).await
                        {
                            let ctrs = get_counters();
                            ctrs.multi
                                .println(format!(
                                    "{} {}: {}",
                                    "Failed to write caption file for".red().bold(),
                                    post.file_name(self.name_type).red().bold(),
                                    error
                                ))
                                .unwrap();
                        };
                    }
                }
            })
            .await
    }

    async fn check_file_exists(
        post: &Post,
        output: &Path,
        name_type: NameType,
    ) -> Result<bool, QueueError> {
        let counters = get_counters();
        let id_name = post.file_name(NameType::ID);
        let md5_name = post.file_name(NameType::MD5);

        let name = post.file_name(name_type);

        let raw_path = output.parent().unwrap();

        let (actual, file_is_same) = match (name_type, output.exists()) {
            (NameType::ID, true) | (NameType::MD5, true) => {
                debug!("File {} found.", &name);
                (output.to_path_buf(), false)
            }
            (NameType::ID, false) => {
                debug!("File {} not found.", &name);
                debug!("Trying possibly matching file: {}", &md5_name);
                (raw_path.join(Path::new(&md5_name)), true)
            }
            (NameType::MD5, false) => {
                debug!("File {} not found.", &name);
                debug!("Trying possibly matching file: {}", &id_name);
                (raw_path.join(Path::new(&id_name)), true)
            }
        };

        if actual.exists() {
            debug!(
                "Found file {}",
                actual.file_name().unwrap().to_str().unwrap()
            );
            let file_digest = compute(read(&actual).await?);
            let hash = format!("{:x}", file_digest);
            if hash == post.md5 {
                if file_is_same {
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

                    return Ok(true);
                }
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

                return Ok(true);
            }
            remove_file(&actual).await?;
            counters.multi.println(format!(
                "{} {} {}",
                "File".bold().red(),
                name.bold().yellow().italic(),
                "MD5 mismatch. Redownloading...".bold().red()
            ))?;

            Ok(false)
        } else {
            Ok(false)
        }
    }

    async fn fetch(
        client: Client,
        variant: ImageBoards,
        post: &Post,
        output: &Path,
        name_type: NameType,
        pool: bool,
    ) -> Result<(), PostError> {
        debug!("Fetching {}", &post.url);

        let counters = get_counters();

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
        let mut stream = res.bytes_stream();

        let buf_size: usize = size.try_into()?;

        let fname = if pool {
            post.seq_file_name(6)
        } else {
            post.file_name(name_type)
        };

        let out = output.join(fname);

        debug!("Creating {:?}", &out);
        let file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(out)
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

        Ok(())
    }
}
