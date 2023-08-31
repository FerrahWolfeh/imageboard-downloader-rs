use ibdl_common::{
    post::tags::{Tag, TagType},
    serde::{self, Deserialize, Serialize},
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "self::serde")]
pub struct DanbooruPoolList {
    pub post_ids: Vec<u64>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "self::serde")]
pub struct DanbooruPost {
    pub id: Option<u64>,
    pub md5: Option<String>,
    pub file_url: Option<String>,
    pub tag_string: Option<String>,
    pub tag_string_general: Option<String>,
    pub tag_string_character: Option<String>,
    pub tag_string_copyright: Option<String>,
    pub tag_string_artist: Option<String>,
    pub tag_string_meta: Option<String>,
    pub file_ext: Option<String>,
    pub rating: Option<String>,
}

impl DanbooruPost {
    pub fn map_tags(&self) -> Vec<Tag> {
        let mut tags = Vec::with_capacity(64);
        if let Some(tagstr) = &self.tag_string_artist {
            tags.extend(tagstr.split(' ').map(|tag| Tag::new(tag, TagType::Author)))
        }
        if let Some(tagstr) = &self.tag_string_copyright {
            tags.extend(
                tagstr
                    .split(' ')
                    .map(|tag| Tag::new(tag, TagType::Copyright)),
            )
        }
        if let Some(tagstr) = &self.tag_string_character {
            tags.extend(
                tagstr
                    .split(' ')
                    .map(|tag| Tag::new(tag, TagType::Character)),
            )
        }
        if let Some(tagstr) = &self.tag_string_general {
            tags.extend(tagstr.split(' ').map(|tag| Tag::new(tag, TagType::General)))
        }
        if let Some(tagstr) = &self.tag_string_meta {
            tags.extend(tagstr.split(' ').map(|tag| Tag::new(tag, TagType::Meta)))
        }

        tags
    }
}
