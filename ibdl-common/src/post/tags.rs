use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tag {
    tag: String,
    tag_type: TagType,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TagType {
    Author,
    Copyright,
    Character,
    /// Exclusive to e621/926
    Species,
    General,
    /// Exclusive to e621/926
    Lore,
    Meta,
    Any,
}

impl Tag {
    pub fn new(text: &str, tag_type: TagType) -> Self {
        Self {
            tag: text.to_string(),
            tag_type,
        }
    }

    pub fn tag(&self) -> String {
        self.tag.clone()
    }

    pub const fn tag_type(&self) -> TagType {
        self.tag_type
    }

    pub const fn is_prompt_tag(&self) -> bool {
        match self.tag_type {
            TagType::Author | TagType::Copyright | TagType::Lore | TagType::Meta => false,
            TagType::Character | TagType::Species | TagType::General | TagType::Any => true,
        }
    }
}
