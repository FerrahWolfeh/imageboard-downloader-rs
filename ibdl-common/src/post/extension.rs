use std::fmt::{Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::error::PostError;

/// Enum representing the 8 possible extensions a downloaded post can have.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Extension {
    AVIF,
    JXL,
    /// The `JPG` variant also encompasses the other extensions a jpeg might have, including `.jpg`, `.jpeg` and `.jfif`
    JPG,
    /// The `PNG` variant can also include the rare `.apng` whenever it's present.
    PNG,
    WEBP,
    GIF,
    WEBM,
    MP4,
    /// Pixiv Ugoira is usually downloaded as a zip file with all frames without any additional metadata.
    Ugoira,
    /// Used for any file whose extension is unknown or not currently supported by this library.
    Unknown,
}

impl Extension {
    /// Naive and simple way of recognizing the extension of a post to use in [`Post`](crate::post::Post). This function never fails.
    pub fn guess_format(s: &str) -> Self {
        let uu = Self::from_str(s);
        if uu.is_err() {
            return Self::Unknown;
        }
        uu.unwrap()
    }

    pub const fn is_video(&self) -> bool {
        matches!(self, Self::GIF | Self::WEBM | Self::MP4 | Self::Ugoira)
    }
}

impl FromStr for Extension {
    type Err = PostError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "jpg" | "jpeg" | "jfif" => Ok(Self::JPG),
            "png" | "apng" => Ok(Self::PNG),
            "webp" => Ok(Self::WEBP),
            "webm" => Ok(Self::WEBM),
            "mp4" => Ok(Self::MP4),
            "gif" => Ok(Self::GIF),
            "zip" => Ok(Self::Ugoira),
            "jxl" => Ok(Self::JXL),
            "avif" => Ok(Self::AVIF),
            _ => Err(PostError::UnknownExtension {
                message: s.to_string(),
            }),
        }
    }
}

impl Display for Extension {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JPG => write!(f, "jpg"),
            Self::PNG => write!(f, "png"),
            Self::WEBP => write!(f, "webp"),
            Self::GIF => write!(f, "gif"),
            Self::WEBM => write!(f, "webm"),
            Self::MP4 => write!(f, "mp4"),
            Self::Ugoira => write!(f, "zip"),
            Self::Unknown => write!(f, "bin"),
            Self::AVIF => write!(f, "avif"),
            Self::JXL => write!(f, "jxl"),
        }
    }
}
