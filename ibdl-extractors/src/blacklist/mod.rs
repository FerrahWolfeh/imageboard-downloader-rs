//! Global post filter
//!
//! # The Global Blacklist
//! Imageboards use tags to categorize posts, making them searchable. The global blacklist
//! provides a mechanism to filter out posts containing unwanted tags before they are added
//! to the download queue.
//!
//! ## Config file
//! The global blacklist is created in `$XDG_CONFIG_HOME/imageboard-downloader/blacklist.toml`
//! (or equivalent OS-specific configuration directory).
//!
//! Users can define tags to be blacklisted in this TOML file:
//! ```toml
//! [blacklist] # This is the main table containing all blacklist configurations
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
//! This configuration allows users to specify tags they wish to avoid. If a post contains
//! any tag listed in the applicable blacklist (global or site-specific), it will be excluded.
use ahash::AHashSet;
use ibdl_common::directories::ProjectDirs;
use ibdl_common::log::{debug, warn};
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

const BF_INIT_TEXT: &str = include_str!("blacklist.toml");

/// Represents a list of blacklisted tags for a specific imageboard or for global application.
/// Used internally for deserializing the `blacklist.toml` file.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "self::serde")]
struct BlacklistTags {
    /// A vector of tags (as strings) to be blacklisted.
    tags: Vec<String>,
}

/// Represents the entire blacklist configuration, loaded from `blacklist.toml`.
/// It contains global blacklisted tags and site-specific blacklisted tags.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "self::serde")]
pub struct GlobalBlacklist {
    /// A map where keys are imageboard names (e.g., "danbooru", "e621") or "global".
    /// The values are `BlacklistTags` structs containing the tags to be excluded for that specific scope.
    ///
    blacklist: HashMap<String, BlacklistTags>,
}

impl GlobalBlacklist {
    /// Parses the blacklist config file and fills the struct. If the file does not exist (deleted
    /// or first run), it will be created.
    ///
    /// # Returns
    /// A `Result` containing the `GlobalBlacklist` instance on success, or an `ExtractorError`
    /// if there's an issue with file operations (creation, reading) or TOML deserialization.
    ///
    /// # Errors
    /// This function can return an error if:
    /// - The configuration directory cannot be accessed or created.
    /// - The `blacklist.toml` file cannot be created or written to.
    /// - The `blacklist.toml` file cannot be read.
    /// - The content of `blacklist.toml` is not valid TOML or does not match the expected structure.
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

        let deserialized = from_str::<Self>(&gbl_string)?;

        debug!("Global blacklist config decoded");
        debug!(
            "Global blacklist setup with {} servers",
            deserialized.blacklist.keys().len().saturating_sub(1)
        );
        Ok(deserialized)
    }
}

/// A filter for `Post` items, capable of removing posts based on blacklisted tags,
/// selected ratings, desired file extension, and animation status.
pub struct BlacklistFilter {
    /// A set of all tags that should be blacklisted for the current operation.
    /// This includes global tags, site-specific tags, and any tags provided through authentication.
    gbl_tags: AHashSet<String>,
    /// A sorted vector of `Rating`s that are allowed. Posts with ratings not in this list will be filtered out.
    /// If empty, no rating filtering is applied.
    selected_ratings: Vec<Rating>,
    /// A flag indicating whether all blacklist filtering (tags) is disabled.
    disabled: bool,
    /// A flag indicating whether animated posts (GIFs, WEBMs, MP4s, or posts tagged 'animated') should be ignored.
    ignore_animated: bool,
    /// An optional `Extension` to filter by. If `Some`, only posts with this extension will be kept.
    extension: Option<Extension>,
}

impl BlacklistFilter {
    /// Creates a new `BlacklistFilter` instance.
    ///
    /// This constructor initializes the filter by loading global and site-specific blacklists,
    /// incorporating authentication-related tags, and setting up filters for ratings,
    /// animated content, and specific file extensions.
    ///
    /// # Arguments
    /// * `imageboard`: The `ServerConfig` for the imageboard being accessed, used to fetch site-specific blacklists.
    /// * `auth_tags`: A slice of tags that should always be blacklisted, typically derived from user authentication or specific session settings.
    /// * `selected_ratings`: A slice of `Rating`s that the user wants to download. Posts with other ratings will be filtered out.
    /// * `disabled`: If `true`, tag-based blacklisting (both global and site-specific) is disabled. Other filters (ratings, extension, animated) still apply.
    /// * `ignore_animated`: If `true`, posts identified as animated (either by tag 'animated' or by video extension) will be filtered out.
    /// * `extension`: If `Some(Extension)`, only posts with the specified file extension will be kept. If `None`, no extension filtering is applied.
    ///
    /// # Returns
    /// A `Result` containing the `BlacklistFilter` on success, or an `ExtractorError` if
    /// the `GlobalBlacklist` cannot be loaded.
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

            gbl.blacklist.get("global").map_or_else(
                || warn!("Global blacklist config has no [blacklist.global] section!"),
                |global| {
                    if global.tags.is_empty() {
                        debug!("Global blacklist is empty");
                    } else {
                        gbl_tags.extend(global.tags.iter().cloned());
                    }
                },
            );

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

    /// Filters a list of `Post`s based on the configured criteria.
    ///
    /// The filtering process is as follows:
    /// 1. If an `extension` is specified, posts not matching it are removed.
    /// 2. If `selected_ratings` is not empty, posts with ratings not in the list are removed.
    /// 3. If blacklisting is not `disabled`:
    ///    a. Posts containing any tag from `gbl_tags` are removed.
    ///    b. If `ignore_animated` is true, posts tagged 'animated' or with video extensions are removed.
    ///
    /// # Arguments
    /// * `list`: A `Vec<Post>` to be filtered.
    ///
    /// # Returns
    /// A tuple containing:
    ///  - `u64`: The total number of posts removed by the filter.
    ///  - `Vec<Post>`: The filtered list of posts.
    #[inline]
    #[must_use]
    pub fn filter(&self, list: Vec<Post>) -> (u64, Vec<Post>) {
        let mut original_list = list;

        let original_size = original_list.len();
        let mut removed = 0;

        let start = Instant::now();
        if let Some(ext) = self.extension {
            debug!("Selecting only posts with extension {:?}", ext.to_string());
            original_list.retain(|post| ext == post.extension);
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

            let bp = if self.gbl_tags.is_empty() {
                0
            } else {
                debug!("Removing posts with tags {:?}", self.gbl_tags);
                original_list.retain(|c| !c.tags.iter().any(|s| self.gbl_tags.contains(&s.tag())));
                fsize - original_list.len()
            };

            if self.ignore_animated {
                original_list.retain(|post| {
                    !(post.tags.contains(&Tag::new("animated", TagType::Meta))
                        || post.extension.is_video())
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
