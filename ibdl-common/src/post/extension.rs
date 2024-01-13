use std::str::FromStr;

use super::error::PostError;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Extension {
    JPG,
    PNG,
    WEBP,
    GIF,
    WEBM,
    MP4,
    Ugoira,
    Unknown,
}

impl Extension {
    pub fn guess_format(s: &str) -> Self {
        let uu = Self::from_str(s);
        if uu.is_err() {
            return Self::Unknown;
        }
        uu.unwrap()
    }
}

impl FromStr for Extension {
    type Err = PostError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "jpg" | "jpeg" | "jfif" => Ok(Self::JPG),
            "png" | "apng" => Ok(Self::PNG),
            "webp" => Ok(Self::WEBP),
            "webm" => Ok(Self::WEBM),
            "mp4" => Ok(Self::MP4),
            "gif" => Ok(Self::GIF),
            "zip" => Ok(Self::Ugoira),
            _ => Err(PostError::UnknownExtension {
                message: s.to_string(),
            }),
        }
    }
}

impl ToString for Extension {
    fn to_string(&self) -> String {
        match self {
            Self::JPG => String::from("jpg"),
            Self::PNG => String::from("png"),
            Self::WEBP => String::from("webp"),
            Self::GIF => String::from("gif"),
            Self::WEBM => String::from("webm"),
            Self::MP4 => String::from("mp4"),
            Self::Ugoira => String::from("zip"),
            Self::Unknown => String::from("bin"),
        }
    }
}
