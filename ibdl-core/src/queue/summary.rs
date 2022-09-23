use super::error::QueueError;
use chrono::{serde::ts_seconds, DateTime, Utc};
use ibdl_common::serde::{self, Deserialize, Serialize};
use ibdl_common::serde_json::from_str;
use ibdl_common::{
    bincode::{deserialize, serialize},
    post::{NameType, Post},
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
use zip::{write::FileOptions, CompressionMethod, ZipArchive, ZipWriter};

/// The download summary can be saved in two formats:
/// - As a ZSTD-compressed bincode file
/// - As a generic JSON file.
#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "self::serde")]
pub enum SummaryType {
    ZSTDBincode,
    JSON,
}

/// The generic information of a [Post](ibdl_common::post) along with the name of the file saved in the output directory.
#[derive(Debug, Serialize, Deserialize, Eq, PartialOrd, Ord)]
#[serde(crate = "self::serde")]
pub struct PostInfo {
    pub saved_as: String,
    pub post: Post,
}

impl PartialEq for PostInfo {
    fn eq(&self, other: &PostInfo) -> bool {
        self.post.id == other.post.id
    }
}

/// The final summary file. It containes common information for the user to read and the necessary data to filter posts in certain occasions.
#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "self::serde")]
pub struct SummaryFile {
    pub file_mode: SummaryType,
    pub imageboard: ImageBoards,
    pub name_mode: NameType,
    pub tags: Vec<String>,
    #[serde(with = "ts_seconds")]
    pub last_updated: DateTime<Utc>,
    pub last_downloaded: u64,
    pub posts: Vec<PostInfo>,
}

impl SummaryFile {
    /// Create a [SummaryFile] from the supplied information about all downloaded posts.
    pub fn new(
        imageboard: ImageBoards,
        tags: &[String],
        posts: &[Post],
        name_mode: NameType,
        file_mode: SummaryType,
    ) -> Self {
        let last_down = posts.first().unwrap().clone();

        let mut post_list: Vec<PostInfo> = Vec::with_capacity(posts.len());

        posts.iter().for_each(|post| {
            let info = PostInfo {
                saved_as: post.file_name(name_mode),
                post: post.clone(),
            };

            post_list.push(info);
        });

        post_list.sort();
        post_list.reverse();

        Self {
            file_mode,
            imageboard,
            name_mode,
            tags: tags.to_vec(),
            last_updated: Utc::now(),
            last_downloaded: last_down.id,
            posts: post_list,
        }
    }

    /// Writes this struct as a summary file inside a supplied zip file.
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

    /// Writes this struct as a summary file in the given [Path].
    pub async fn write_summary(&self, path: &Path) -> Result<(), QueueError> {
        let mut dsum = AsyncFile::create(path).await?;

        let string = self.to_bincode()?;

        dsum.write_all(&string).await?;

        Ok(())
    }

    /// Read the summary file from the supplied [Path].
    pub async fn read_summary(path: &Path, summary_type: SummaryType) -> Result<Self, QueueError> {
        let mut raw_data: Vec<u8> = vec![];
        let mut dsum = AsyncFile::open(path).await?;

        dsum.read_to_end(&mut raw_data).await?;

        match summary_type {
            SummaryType::ZSTDBincode => Ok(Self::from_bincode(&raw_data)?),
            SummaryType::JSON => Ok(Self::from_json_slice(&raw_data)?),
        }
    }

    /// Read the bincode summary and decode it into a [SummaryFile]
    #[inline]
    pub fn from_bincode(slice: &[u8]) -> Result<Self, QueueError> {
        match deserialize::<Self>(&decode_all(slice)?) {
            Ok(summary) => Ok(summary),
            Err(err) => Err(QueueError::SummaryDeserializeFail {
                error: err.to_string(),
            }),
        }
    }

    /// Read the summary as a raw JSON slice from and decode it into a [SummaryFile]
    #[inline]
    pub fn from_json_slice(slice: &[u8]) -> Result<Self, QueueError> {
        match from_slice::<SummaryFile>(slice) {
            Ok(sum) => Ok(sum),
            Err(error) => Err(QueueError::SummaryDeserializeFail {
                error: error.to_string(),
            }),
        }
    }

    /// Read the summary as a raw JSON string and decode it into a [SummaryFile]
    #[inline]
    pub fn from_json_str(text: &str) -> Result<Self, QueueError> {
        match from_str::<SummaryFile>(text) {
            Ok(sum) => Ok(sum),
            Err(error) => Err(QueueError::SummaryDeserializeFail {
                error: error.to_string(),
            }),
        }
    }

    pub async fn read_zip_summary(
        path: &Path,
        summary_type: SummaryType,
    ) -> Result<Self, QueueError> {
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

            match summary_type {
                SummaryType::ZSTDBincode => Ok(Self::from_bincode(&summary_slice)?),
                SummaryType::JSON => Ok(Self::from_json_slice(&summary_slice)?),
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
