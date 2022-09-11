use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};
use tokio::{
    fs::File as AsyncFile,
    io::{AsyncReadExt, AsyncWriteExt},
    task::spawn_blocking,
};

use bincode::{deserialize, serialize};
use chrono::{serde::ts_seconds, DateTime, Utc};
use serde::{Deserialize, Serialize};
use zip::{write::FileOptions, CompressionMethod, ZipArchive, ZipWriter};
use zstd::{decode_all, encode_all};

use crate::Post;

use super::error::QueueError;

#[derive(Debug, Serialize, Deserialize)]
pub struct SummaryFile {
    #[serde(with = "ts_seconds")]
    pub last_updated: DateTime<Utc>,
    pub last_downloaded: Post,
    pub posts: Vec<Post>,
}

impl SummaryFile {
    pub fn new(posts: Vec<Post>) -> Self {
        let last_down = posts.first().unwrap().clone();

        Self {
            last_updated: Utc::now(),
            last_downloaded: last_down,
            posts,
        }
    }

    pub fn write_zip_summary(&self, zip: &mut ZipWriter<File>) -> Result<(), QueueError> {
        let serialized = serde_json::to_string_pretty(self)?;

        zip.start_file(
            "00_summary.json",
            FileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .compression_level(Some(9)),
        )?;

        zip.write_all(serialized.as_bytes())?;
        Ok(())
    }

    pub async fn write_summary(&self, path: &Path) -> Result<(), QueueError> {
        let mut dsum = AsyncFile::create(path).await?;

        let string = match serialize(&self) {
            Ok(data) => encode_all(&*data, 9)?,
            Err(err) => {
                return Err(QueueError::BinarySerializeFail {
                    error: err.to_string(),
                })
            }
        };

        dsum.write_all(&string).await?;

        Ok(())
    }

    pub async fn read_summary(path: &Path) -> Result<Self, QueueError> {
        let mut raw_data: Vec<u8> = vec![];
        let mut dsum = AsyncFile::open(path).await?;

        dsum.read_to_end(&mut raw_data).await?;

        match deserialize::<Self>(&decode_all(&*raw_data)?) {
            Ok(summary) => Ok(summary),
            Err(err) => Err(QueueError::SummaryDeserializeError {
                error: err.to_string(),
            }),
        }
    }

    pub async fn read_zip_summary(path: &Path) -> Result<Self, QueueError> {
        let path = path.to_path_buf();
        spawn_blocking(move || -> Result<Self, QueueError> {
            let file = File::open(&path)?;
            let mut zip = ZipArchive::new(file)?;
            let mut raw_bytes = match zip.by_name("00_summary.json") {
                Ok(bytes) => bytes,
                Err(_) => {
                    return Err(QueueError::ZipSummaryReadError {
                        file: path.display().to_string(),
                    })
                }
            };

            let mut summary_slice = vec![];

            raw_bytes.read_to_end(&mut summary_slice)?;

            match serde_json::from_slice::<SummaryFile>(&summary_slice) {
                Ok(sum) => Ok(sum),
                Err(error) => Err(QueueError::SummaryDeserializeError {
                    error: error.to_string(),
                }),
            }
        })
        .await
        .unwrap()
    }
}
