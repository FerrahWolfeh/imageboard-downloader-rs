use ibdl_common::{
    post::tags::{Tag, TagType},
    serde::{self, Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "self::serde")]
pub struct GelbooruTopLevel {
    pub post: Vec<GelbooruPost>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "self::serde")]
pub struct GelbooruPost {
    pub id: Option<u64>,
    pub md5: Option<String>,
    pub file_url: Option<String>,
    pub tags: Option<String>,
    pub rating: Option<String>,
}

impl GelbooruPost {
    pub fn map_tags(&self) -> Vec<Tag> {
        let mut tags = Vec::with_capacity(64);
        if let Some(tagstr) = &self.tags {
            tags.extend(tagstr.split(' ').map(|tag| Tag::new(tag, TagType::Any)));
        }

        tags
    }
}
