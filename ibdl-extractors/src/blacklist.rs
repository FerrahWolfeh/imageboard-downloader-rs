//! Global post filter
//!
//! # The Global Blacklist
//! Imageboards tag their posts in order to facilitate searching,
//! the global blacklist implements a filter to exclude from the download queue all posts with unwanted tags.
//!
//! ## Config file
//! The global blacklist is created in `$XDG_CONFIG_HOME/imageboard-downloader/blacklist.toml`
//!
//! The user can define the tags as follows
//! ```toml
//! [blacklist]
//! global = ["tag_1", "tag_2"] # Place in this array all the tags that will be excluded from all imageboards
//!
//! # Place in the following all the tags that will be excluded from specific imageboards
//!
//! danbooru = ["tag_3", "tag_4"] # Will exclude these tags only when downloading from Danbooru
//!
//! e621 = []
//!
//! rule34 = []
//!
//! realbooru = []
//!
//! gelbooru = []
//!
//! konachan = []
//! ```
//!
//! With this, the user can input all tags that they do not want to download. In case a post has
//! any of the tags set in the blacklist, it will be removed from the download queue.
use ahash::AHashSet;
use ibdl_common::directories::ProjectDirs;
use ibdl_common::log::debug;
use ibdl_common::post::extension::Extension;
use ibdl_common::post::rating::Rating;
use ibdl_common::post::tags::{Tag, TagType};
use ibdl_common::post::Post;
use ibdl_common::serde::{self, Deserialize, Serialize};
use ibdl_common::tokio::fs::{create_dir_all, read_to_string, File};
use ibdl_common::tokio::io::AsyncWriteExt;
use ibdl_common::tokio::time::Instant;
use std::collections::HashMap;
use std::path::Path;
use toml::from_str;

use crate::extractor_config::ServerConfig;

use super::error::ExtractorError;

static VIDEO_EXTENSIONS: [&str; 4] = ["mp4", "zip", "webm", "gif"];

const BF_INIT_TEXT: &str = r"[blacklist]
[blacklist.global] 
tags = [] # Place in this array all the tags that will be excluded from all imageboards

# Place in the following all the tags that will be excluded from specific imageboards 

[blacklist.danbooru]
tags = []

[blacklist.e621]
tags = []

[blacklist.realbooru]
tags = []

[blacklist.rule34]
tags = []

[blacklist.gelbooru]
tags = []

[blacklist.konachan]
tags = []
";

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "self::serde")]
struct BlacklistTags {
    tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "self::serde")]
pub struct GlobalBlacklist {
    /// In this array, the user will declare tags that should be excluded from all imageboards
    blacklist: HashMap<String, BlacklistTags>,
}

impl GlobalBlacklist {
    /// Parses the blacklist config file and fills the struct. If the file does not exist (deleted
    /// or first run), it will be created.
    pub async fn get() -> Result<Self, ExtractorError> {
        let cache_dir = ProjectDirs::from("com", "ferrahwolfeh", "imageboard-downloader").unwrap();

        let cfold = cache_dir.config_dir();

        if !cfold.exists() {
            create_dir_all(cfold).await?;
        }

        let dir = cfold.join(Path::new("blacklist.toml"));

        if !dir.exists() {
            debug!("Creating blacklist file");
            File::create(&dir)
                .await?
                .write_all(BF_INIT_TEXT.as_bytes())
                .await?;
        }

        let gbl_string = read_to_string(&dir).await?;

        let Ok(deserialized) = from_str::<Self>(&gbl_string) else {
            return Err(ExtractorError::BlacklistDecodeError {
                path: dir.display().to_string(),
            });
        };

        debug!("Global blacklist config decoded");
        debug!(
            "Global blacklist setup with {} servers",
            deserialized.blacklist.keys().len() - 1
        );
        Ok(deserialized)
    }
}

pub struct BlacklistFilter {
    gbl_tags: AHashSet<String>,
    selected_ratings: Vec<Rating>,
    disabled: bool,
    ignore_animated: bool,
    extension: Option<Extension>,
}

impl BlacklistFilter {
    pub async fn new(
        imageboard: ServerConfig,
        auth_tags: &[String],
        selected_ratings: &[Rating],
        disabled: bool,
        ignore_animated: bool,
        extension: Option<Extension>,
    ) -> Result<Self, ExtractorError> {
        let mut gbl_tags: AHashSet<String> = AHashSet::new();
        if !disabled {
            gbl_tags.extend(auth_tags.iter().cloned());

            let gbl = GlobalBlacklist::get().await?;

            if let Some(global) = gbl.blacklist.get(&"global".to_string()) {
                if global.tags.is_empty() {
                    debug!("Global blacklist is empty");
                } else {
                    gbl_tags.extend(global.tags.iter().cloned());
                }
            }

            if let Some(special) = gbl.blacklist.get(&imageboard.name) {
                if !special.tags.is_empty() {
                    debug!("{} blacklist: {:?}", imageboard.pretty_name, &special.tags);
                    gbl_tags.extend(special.tags.iter().cloned());
                }
            }
        }

        let mut sorted_list = selected_ratings.to_vec();
        sorted_list.sort();

        Ok(Self {
            gbl_tags,
            selected_ratings: sorted_list,
            disabled,
            ignore_animated,
            extension,
        })
    }

    #[inline]
    #[must_use]
    pub fn filter(&self, list: Vec<Post>) -> (u64, Vec<Post>) {
        let mut original_list = list;

        let original_size = original_list.len();
        let mut removed = 0;

        let ve = AHashSet::from(VIDEO_EXTENSIONS);

        let start = Instant::now();
        if let Some(ext) = self.extension {
            debug!("Selecting only posts with extension {:?}", ext.to_string());
            original_list.retain(|post| ext == Extension::guess_format(&post.extension));
        }

        if !self.selected_ratings.is_empty() {
            debug!("Selected ratings: {:?}", self.selected_ratings);
            original_list.retain(|c| self.selected_ratings.binary_search(&c.rating).is_ok());

            let safe_counter = original_size - original_list.len();
            debug!("Removed {} posts with non-selected ratings", safe_counter);

            removed += safe_counter as u64;
        }

        if !self.disabled {
            let fsize = original_list.len();

            let bp = if !self.gbl_tags.is_empty() {
                debug!("Removing posts with tags {:?}", self.gbl_tags);
                original_list.retain(|c| !c.tags.iter().any(|s| self.gbl_tags.contains(&s.tag())));
                fsize - original_list.len()
            } else {
                0
            };

            if self.ignore_animated {
                original_list.retain(|post| {
                    let ext = post.extension.as_str();
                    !(post.tags.contains(&Tag::new("animated", TagType::Meta)) || ve.contains(ext))
                });
            }

            debug!("Blacklist removed {} posts", bp);
            removed += bp as u64;
        }

        debug!("Filtering took {:?}", start.elapsed());
        debug!("Removed total of {} posts", removed);

        (removed, original_list)
    }
}
