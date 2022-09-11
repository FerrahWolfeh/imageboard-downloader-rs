//! Global post filter
//!
//! # The Global Blacklist
//! Imageboard websites tag their posts in order to facilitate searching,
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
use std::path::Path;

use ahash::AHashSet;
use anyhow::Context;
use cfg_if::cfg_if;
use directories::ProjectDirs;
use log::debug;
use serde::{Deserialize, Serialize};
use tokio::fs::{create_dir_all, read_to_string, File};
use tokio::io::AsyncWriteExt;
use tokio::time::Instant;
use toml::from_str;

use crate::imageboards::post::rating::Rating;
use crate::{ImageBoards, Post};

use super::error::ExtractorError;

const BF_INIT_TEXT: &[u8; 275] = br#"[blacklist]
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
pub struct BlacklistCategories {
    pub global: AHashSet<String>,
    pub danbooru: AHashSet<String>,
    pub e621: AHashSet<String>,
    pub realbooru: AHashSet<String>,
    pub rule34: AHashSet<String>,
    pub gelbooru: AHashSet<String>,
    pub konachan: AHashSet<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GlobalBlacklist {
    /// In this array, the user will declare tags that should be excluded from all imageboards
    pub blacklist: Option<BlacklistCategories>,
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
            File::create(&dir).await?.write_all(BF_INIT_TEXT).await?;
        }

        let gbl_string = read_to_string(&dir).await?;
        let deserialized = match from_str::<Self>(&gbl_string)
            .with_context(|| "Failed parsing the blacklist file.")
        {
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
    imageboard: ImageBoards,
    auth_tags: AHashSet<String>,
    gbl_tags: AHashSet<String>,
    safe: bool,
    disabled: bool,
}

impl BlacklistFilter {
    pub async fn init(
        imageboard: ImageBoards,
        auth_tags: &AHashSet<String>,
        safe: bool,
        disabled: bool,
    ) -> Result<Self, ExtractorError> {
        if safe {
            debug!("Safe mode active");
        }

        let mut gbl_tags: AHashSet<String> = AHashSet::new();
        if !disabled {
            cfg_if! {
                if #[cfg(feature = "global_blacklist")] {
                    let gbl = GlobalBlacklist::get().await?;

                    if let Some(tags) = gbl.blacklist {
                        if tags.global.is_empty() {
                            debug!("Global blacklist is empty");
                        } else {
                            gbl_tags.extend(tags.global);
                        }

                        let special_tags = match imageboard {
                                ImageBoards::Danbooru => {
                                    tags.danbooru
                                }
                                ImageBoards::E621 => {
                                    tags.e621
                                }
                                ImageBoards::Rule34 => {
                                    tags.rule34
                                }
                                ImageBoards::Gelbooru => {
                                    tags.gelbooru
                                }
                                ImageBoards::Realbooru => {
                                    tags.realbooru
                                }
                                ImageBoards::Konachan => {
                                    tags.konachan
                                }
                            };

                            if !special_tags.is_empty() {
                                gbl_tags.extend(special_tags);
                            }
                    }
                }
            }
        }

        Ok(Self {
            imageboard,
            auth_tags: auth_tags.clone(),
            gbl_tags,
            safe,
            disabled,
        })
    }

    #[inline]
    pub fn filter(&self, list: Vec<Post>) -> (u64, Vec<Post>) {
        let mut original_list = list;

        let original_size = original_list.len();
        let mut removed = 0;

        let start = Instant::now();
        if self.safe {
            original_list.retain(|c| c.rating == Rating::Safe);

            let safe_counter = original_size - original_list.len();
            debug!("Safe mode removed {} posts", safe_counter);

            removed += safe_counter as u64;
        }

        if !self.disabled {
            if !self.auth_tags.is_empty()
                && matches!(self.imageboard, ImageBoards::Danbooru | ImageBoards::E621)
            {
                let secondary_sz = original_list.len();
                original_list.retain(|c| !c.tags.iter().any(|s| self.auth_tags.contains(s)));

                let bp = secondary_sz - original_list.len();
                debug!("User blacklist removed {} posts", bp);
                removed += bp as u64;
            }

            cfg_if! {
                if #[cfg(feature = "global_blacklist")] {
                    if !self.gbl_tags.is_empty() {
                        let fsize = original_list.len();
                        debug!("Removing posts with tags [{:?}]", self.gbl_tags);

                        original_list.retain(|c| !c.tags.iter().any(|s| self.gbl_tags.contains(s)));

                        let bp = fsize - original_list.len();
                        debug!("Global blacklist removed {} posts", bp);
                        removed += bp as u64;
                    }
                }
            }
        }

        let end = Instant::now();
        debug!("Filtering took {:?}", end - start);
        debug!("Removed total of {} posts", removed);

        (removed, original_list)
    }
}
