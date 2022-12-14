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
use ibdl_common::post::rating::Rating;
use ibdl_common::post::Post;
use ibdl_common::serde::{self, Deserialize, Serialize};
use ibdl_common::tokio::fs::{create_dir_all, read_to_string, File};
use ibdl_common::tokio::io::AsyncWriteExt;
use ibdl_common::tokio::time::Instant;
use ibdl_common::ImageBoards;
use std::path::Path;
use toml::from_str;

use super::error::ExtractorError;

const BF_INIT_TEXT: &str = r#"[blacklist]
global = [] # Place in this array all the tags that will be excluded from all imageboards

# Place in the following all the tags that will be excluded from specific imageboards 

danbooru = []

e621 = []

realbooru = []

rule34 = []

gelbooru = []

konachan = []
"#;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "self::serde")]
struct BlacklistCategories {
    global: Vec<String>,
    danbooru: Vec<String>,
    e621: Vec<String>,
    realbooru: Vec<String>,
    rule34: Vec<String>,
    gelbooru: Vec<String>,
    konachan: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "self::serde")]
pub struct GlobalBlacklist {
    /// In this array, the user will declare tags that should be excluded from all imageboards
    blacklist: Option<BlacklistCategories>,
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
        let deserialized = match from_str::<Self>(&gbl_string) {
            Ok(data) => data,
            Err(_) => {
                return Err(ExtractorError::BlacklistDecodeError {
                    path: dir.display().to_string(),
                })
            }
        };
        debug!("Global blacklist decoded");
        Ok(deserialized)
    }
}

pub struct BlacklistFilter {
    gbl_tags: AHashSet<String>,
    selected_ratings: Vec<Rating>,
    disabled: bool,
}

impl BlacklistFilter {
    pub async fn init(
        imageboard: ImageBoards,
        auth_tags: &[String],
        selected_ratings: &[Rating],
        disabled: bool,
    ) -> Result<Self, ExtractorError> {
        let mut gbl_tags: AHashSet<String> = AHashSet::new();
        if !disabled {
            gbl_tags.extend(auth_tags.iter().cloned());

            let gbl = GlobalBlacklist::get().await?;

            if let Some(tags) = gbl.blacklist {
                if tags.global.is_empty() {
                    debug!("Global blacklist is empty");
                } else {
                    gbl_tags.extend(tags.global);
                }

                let special_tags = match imageboard {
                    ImageBoards::Danbooru => tags.danbooru,
                    ImageBoards::E621 => tags.e621,
                    ImageBoards::Rule34 => tags.rule34,
                    ImageBoards::Gelbooru => tags.gelbooru,
                    ImageBoards::Realbooru => tags.realbooru,
                    ImageBoards::Konachan => tags.konachan,
                };

                if !special_tags.is_empty() {
                    debug!("{} blacklist: {:?}", imageboard.to_string(), &special_tags);
                    gbl_tags.extend(special_tags);
                }
            }
        }

        let mut sorted_list = selected_ratings.to_vec();
        sorted_list.sort();

        Ok(Self {
            gbl_tags,
            selected_ratings: sorted_list,
            disabled,
        })
    }

    #[inline]
    pub fn filter(&self, list: Vec<Post>) -> (u64, Vec<Post>) {
        let mut original_list = list;

        let original_size = original_list.len();
        let mut removed = 0;

        let start = Instant::now();
        if !self.selected_ratings.is_empty() {
            debug!("Selected ratings: {:?}", self.selected_ratings);
            original_list.retain(|c| self.selected_ratings.binary_search(&c.rating).is_ok());

            let safe_counter = original_size - original_list.len();
            debug!("Removed {} posts with non-selected ratings", safe_counter);

            removed += safe_counter as u64;
        }

        if !self.disabled && !self.gbl_tags.is_empty() {
            let fsize = original_list.len();
            debug!("Removing posts with tags {:?}", self.gbl_tags);

            original_list.retain(|c| !c.tags.iter().any(|s| self.gbl_tags.contains(s)));

            let bp = fsize - original_list.len();
            debug!("Blacklist removed {} posts", bp);
            removed += bp as u64;
        }

        let end = Instant::now();
        debug!("Filtering took {:?}", end - start);
        debug!("Removed total of {} posts", removed);

        (removed, original_list)
    }
}
