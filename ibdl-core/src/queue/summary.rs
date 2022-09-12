use ibdl_common::{
    bincode::{deserialize, serialize},
    post::Post,
    serde_json::{from_slice, to_string_pretty},
    tokio,
    zstd::{decode_all, encode_all},
    ImageBoards,
};
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

use chrono::{serde::ts_seconds, DateTime, Utc};
use ibdl_common::serde::{self, Deserialize, Serialize};
use zip::{write::FileOptions, CompressionMethod, ZipArchive, ZipWriter};

use super::error::QueueError;

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "self::serde")]
pub struct SummaryFile {
    pub imageboard: ImageBoards,
    pub tags: Vec<String>,
    #[serde(with = "ts_seconds")]
    pub last_updated: DateTime<Utc>,
    pub last_downloaded: u64,
    pub posts: Vec<Post>,
}

impl SummaryFile {
    pub fn new(imageboard: ImageBoards, tags: Vec<String>, posts: Vec<Post>) -> Self {
        let last_down = posts.first().unwrap().clone();

        Self {
            imageboard,
            tags,
            last_updated: Utc::now(),
            last_downloaded: last_down.id,
            posts,
        }
    }

    pub fn write_zip_summary(&self, zip: &mut ZipWriter<File>) -> Result<(), QueueError> {
        let serialized = self.to_json()?;

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

        let string = self.to_bincode()?;

        dsum.write_all(&string).await?;

        Ok(())
    }

    pub async fn read_summary(path: &Path) -> Result<Self, QueueError> {
        let mut raw_data: Vec<u8> = vec![];
        let mut dsum = AsyncFile::open(path).await?;

        dsum.read_to_end(&mut raw_data).await?;

        match deserialize::<Self>(&decode_all(&*raw_data)?) {
            Ok(summary) => Ok(summary),
            Err(err) => Err(QueueError::SummaryDeserializeFail {
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

            match from_slice::<SummaryFile>(&summary_slice) {
                Ok(sum) => Ok(sum),
                Err(error) => Err(QueueError::SummaryDeserializeFail {
                    error: error.to_string(),
                }),
            }
        })
        .await
        .unwrap()
    }

    #[inline]
    pub fn to_json(&self) -> Result<String, QueueError> {
        match to_string_pretty(self) {
            Ok(json) => Ok(json),
            Err(err) => Err(QueueError::SummarySerializeFail {
                error: err.to_string(),
            }),
        }
    }

    #[inline]
    pub fn to_bincode(&self) -> Result<Vec<u8>, QueueError> {
        match serialize(&self) {
            Ok(data) => Ok(encode_all(&*data, 9)?),
            Err(err) => Err(QueueError::SummarySerializeFail {
                error: err.to_string(),
            }),
        }
    }
}
