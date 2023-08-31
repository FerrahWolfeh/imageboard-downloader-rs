use ibdl_common::{
    post::tags::{Tag, TagType},
    serde::{self, Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "self::serde")]
pub struct E621TopLevel {
    pub posts: Vec<E621Post>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "self::serde")]
pub struct E621Post {
    pub id: Option<u64>,
    pub file: E621File,
    pub tags: Tags,
    pub rating: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "self::serde")]
pub struct E621File {
    pub ext: Option<String>,
    pub md5: Option<String>,
    pub url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "self::serde")]
pub struct E621AuthUser {
    pub success: Option<bool>,
    pub message: Option<String>,
    pub id: Option<u64>,
    pub name: Option<String>,
    pub blacklisted_tags: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "self::serde")]
pub struct Tags {
    pub general: Vec<String>,
    pub species: Vec<String>,
    pub character: Vec<String>,
    pub copyright: Vec<String>,
    pub artist: Vec<String>,
    pub lore: Vec<String>,
    pub meta: Vec<String>,
}

impl Tags {
    pub fn map_tags(&self) -> Vec<Tag> {
        let mut tag_list = Vec::with_capacity(64);
        tag_list.extend(self.general.iter().map(|t| Tag::new(t, TagType::General)));
        tag_list.extend(self.species.iter().map(|t| Tag::new(t, TagType::Species)));
        tag_list.extend(
            self.character
                .iter()
                .map(|t| Tag::new(t, TagType::Character)),
        );
        tag_list.extend(
            self.copyright
                .iter()
                .map(|t| Tag::new(t, TagType::Copyright)),
        );
        tag_list.extend(self.artist.iter().map(|t| Tag::new(t, TagType::Author)));
        tag_list.extend(self.lore.iter().map(|t| Tag::new(t, TagType::Lore)));
        tag_list.extend(self.meta.iter().map(|t| Tag::new(t, TagType::Meta)));

        tag_list
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "self::serde")]
pub struct E621PoolList {
    pub post_ids: Vec<u64>,
}
