//! # Post Tags Module
//!
//! This module defines structures for representing and categorizing tags
//! associated with imageboard posts. Tags are a fundamental part of how
//! imageboards organize and allow searching of content.
//!
//! The primary structures are:
//! - [`Tag`](crate::post::tags::Tag): Represents a single tag, containing its textual content and its type.
//! - [`TagType`](crate::post::tags::TagType): An enum categorizing the nature of a tag (e.g., artist, character, species).

use serde::{Deserialize, Serialize};

/// Represents a single tag associated with an imageboard post.
///
/// Each tag has textual content and a [`TagType`] that categorizes it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tag {
    /// The textual content of the tag (e.g., "blue_sky", "solo_focus").
    tag: String,
    /// The category or type of the tag.
    tag_type: TagType,
}

/// Categorizes the type or nature of a `Tag`.
///
/// Different imageboards might use different sets of tag types, or imply them
/// through prefixes or color-coding. This enum aims to provide a common
/// representation.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TagType {
    /// Tags identifying the artist(s) of the work.
    Author,
    /// Tags related to copyright, series, or franchise.
    Copyright,
    /// Tags identifying specific characters depicted.
    Character,
    /// Tags identifying the species of characters, primarily used on e621/e926.
    Species,
    /// General descriptive tags about the content, scene, or attributes.
    General,
    /// Tags related to lore or setting, primarily used on e621/e926.
    Lore,
    /// Meta-tags related to the post itself (e.g., "high_resolution", "tagme").
    Meta,
    /// A catch-all or unspecified tag type.
    Any,
}

impl Tag {
    /// Creates a new `Tag`.
    ///
    /// # Arguments
    /// * `text`: The textual content of the tag.
    /// * `tag_type`: The [`TagType`] categorizing this tag.
    pub fn new(text: &str, tag_type: TagType) -> Self {
        Self {
            tag: text.to_string(),
            tag_type,
        }
    }

    /// Returns a reference to the textual content of the tag.
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// Returns the [`TagType`] of the tag.
    pub const fn tag_type(&self) -> TagType {
        self.tag_type
    }

    /// Determines if the tag is suitable for use in generating prompts or for general content filtering.
    ///
    /// Tags like `Author`, `Copyright`, `Lore`, and `Meta` are often excluded
    /// as they describe the artwork's context rather than its visual content.
    pub const fn is_prompt_tag(&self) -> bool {
        match self.tag_type {
            TagType::Author | TagType::Copyright | TagType::Lore | TagType::Meta => false,
            TagType::Character | TagType::Species | TagType::General | TagType::Any => true,
        }
    }
}
