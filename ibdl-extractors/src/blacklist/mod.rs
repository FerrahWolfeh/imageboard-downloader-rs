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
use ibdl_common::post::extension::Extension;
use ibdl_common::post::rating::Rating;
use ibdl_common::post::tags::{Tag, TagType};
use ibdl_common::post::Post;
use ibdl_common::ImageBoards;
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::time::Instant;

const BF_INIT_TEXT: &str = include_str!("blacklist.toml");

/// Public constant for the default blacklist.toml content.
/// The main application can use this to create a default config file.
pub const DEFAULT_BLACKLIST_TOML: &str = BF_INIT_TEXT;

/// Represents a list of blacklisted tags for a specific imageboard or for global application.
/// Used internally for deserializing the `blacklist.toml` file.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct BlacklistTags {
    /// A vector of tags (as strings) to be blacklisted.
    tags: Vec<String>,
}

/// Represents the entire blacklist configuration, loaded from `blacklist.toml`.
/// It contains global blacklisted tags and site-specific blacklisted tags.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GlobalBlacklist {
    /// A map where keys are imageboard names (e.g., "danbooru", "e621") or "global".
    /// The values are `BlacklistTags` structs containing the tags to be excluded for that specific scope.
    ///
    blacklist: HashMap<String, BlacklistTags>,
}

impl GlobalBlacklist {
    /// Parses the blacklist configuration from a string.
    ///
    /// # Arguments
    /// * `config_content`: A string slice containing the TOML blacklist configuration.
    ///
    /// # Returns
    /// A `Result` containing the `GlobalBlacklist` instance on success, or a `toml::de::Error`
    /// if deserialization fails.
    ///
    /// # Errors
    /// This function can return an error if:
    /// - The content of `blacklist.toml` is not valid TOML or does not match the expected structure.
    pub fn from_config(config_content: &str) -> Result<Self, toml::de::Error> {
        let deserialized = toml::from_str::<Self>(config_content)?;

        debug!("Global blacklist config decoded");
        debug!(
            "Global blacklist setup with {} servers",
            deserialized.blacklist.keys().len().saturating_sub(1) // Subtract 1 for the "global" key
        );
        Ok(deserialized)
    }
}

/// A filter for `Post` items, capable of removing posts based on blacklisted tags,
/// selected ratings, desired file extension, and animation status.
#[derive(Debug, Clone)]
pub struct BlacklistFilter {
    /// A set of all tags that should be blacklisted for the current operation.
    /// This includes global tags, site-specific tags, and any tags provided through authentication.
    gbl_tags: AHashSet<String>,
    /// A sorted vector of `Rating`s that are allowed. Posts with ratings not in this list will be filtered out.
    selected_ratings: Vec<Rating>,
    /// A flag indicating whether tag-based blacklisting (global, site-specific, user-added) is disabled.
    /// Other filters (ratings, extension, animated) still apply.
    tag_blacklisting_disabled: bool,
    /// A flag indicating whether animated posts (GIFs, WEBMs, MP4s, or posts tagged 'animated') should be ignored.
    ignore_animated: bool,
    /// An optional `Extension` to filter by. If `Some`, only posts with this extension will be kept.
    extension: Option<Extension>,
}

impl BlacklistFilter {
    /// Creates a new `BlacklistFilter` instance.
    ///
    /// This constructor initializes the filter by using a pre-loaded `GlobalBlacklist` configuration,
    /// incorporating additional tags (e.g., from user input or authentication), and setting up filters for ratings,
    /// animated content, and specific file extensions.
    ///
    /// # Arguments
    /// * `global_blacklist_config`: A reference to the parsed `GlobalBlacklist` data.
    /// * `imageboard_name`: The name of the imageboard (e.g., "danbooru") to look up site-specific blacklisted tags.
    /// * `imageboard_pretty_name`: The user-friendly name of the imageboard, for logging purposes.
    /// * `additional_tags_to_blacklist`: A slice of tags (e.g., from user input, command-line arguments, or auth data) to be blacklisted.
    /// * `selected_ratings`: A slice of `Rating`s that the user wants to download. Posts with other ratings will be filtered out.
    /// * `disable_tag_blacklisting`: If `true`, all tag-based blacklisting (from `global_blacklist_config` and `additional_tags_to_blacklist`) is disabled.
    /// * `ignore_animated`: If `true`, posts identified as animated (either by tag 'animated' or by video extension) will be filtered out.
    /// * `extension`: If `Some(Extension)`, only posts with the specified file extension will be kept. If `None`, no extension filtering is applied.
    #[must_use]
    pub fn new(
        global_blacklist_config: &GlobalBlacklist,
        imageboard: ImageBoards,
        additional_tags_to_blacklist: &[String],
        selected_ratings: &[Rating],
        disable_tag_blacklisting: bool,
        ignore_animated: bool,
        extension: Option<Extension>,
    ) -> Self {
        let mut gbl_tags: AHashSet<String> = AHashSet::new();

        // Add tags passed directly to this function (e.g., user-excluded tags, auth tags)
        // These are added regardless of the `disable_tag_blacklisting` flag for global/site lists,
        // as they represent explicit immediate exclusions.
        // However, the overall filtering logic later checks `self.tag_blacklisting_disabled`.
        // To be consistent, if `disable_tag_blacklisting` is true, no tags should be added to `gbl_tags`.
        if !disable_tag_blacklisting {
            gbl_tags.extend(additional_tags_to_blacklist.iter().cloned());

            // Add tags from the global blacklist configuration
            global_blacklist_config.blacklist.get("global").map_or_else(
                || warn!("Global blacklist config has no [blacklist.global] section!"),
                |global| {
                    if global.tags.is_empty() {
                        debug!("Global blacklist (from config) is empty");
                    } else {
                        gbl_tags.extend(global.tags.iter().cloned());
                    }
                },
            );

            let imageboard_name = imageboard.to_string();

            if let Some(special) = global_blacklist_config
                .blacklist
                .get(&imageboard_name.to_lowercase())
            {
                if !special.tags.is_empty() {
                    debug!(
                        "{} site-specific blacklist: {:?}",
                        imageboard_name, &special.tags
                    );
                    gbl_tags.extend(special.tags.iter().cloned());
                }
            }
        }

        let mut sorted_list = selected_ratings.to_vec();
        sorted_list.sort();

        Self {
            gbl_tags,
            selected_ratings: sorted_list,
            tag_blacklisting_disabled: disable_tag_blacklisting,
            ignore_animated,
            extension,
        }
    }

    /// Filters a list of `Post`s based on the configured criteria.
    ///
    /// The filtering process is as follows:
    /// 1. If an `extension` is specified, posts not matching it are removed.
    /// 2. If `selected_ratings` is not empty, posts with ratings not in the list are removed.
    /// 3. If blacklisting is not `disabled`:
    ///    a. If `tag_blacklisting_disabled` is false, posts containing any tag from `gbl_tags` are removed.
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
            debug!("Removed {safe_counter} posts with non-selected ratings");

            removed += safe_counter as u64;
        }

        if !self.tag_blacklisting_disabled {
            let fsize = original_list.len();

            let bp = if self.gbl_tags.is_empty() {
                0
            } else {
                debug!("Removing posts with tags {:?}", self.gbl_tags);
                original_list.retain(|c| !c.tags.iter().any(|s| self.gbl_tags.contains(s.tag())));
                fsize - original_list.len()
            };

            debug!("Blacklist removed {bp} posts");
            removed += bp as u64;
        }
        // Animation filter should apply regardless of tag_blacklisting_disabled,
        // as it's a separate filter criteria.
        if self.ignore_animated {
            let count_before_anim_filter = original_list.len();
            original_list.retain(|post| {
                !(post.tags.contains(&Tag::new("animated", TagType::Meta))
                    || post.extension.is_video())
            });
            removed += (count_before_anim_filter - original_list.len()) as u64;
        }

        debug!("Filtering took {:?}", start.elapsed());
        debug!("Removed total of {removed} posts");

        (removed, original_list)
    }
}
