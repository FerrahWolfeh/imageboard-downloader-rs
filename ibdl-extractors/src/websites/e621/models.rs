use ibdl_common::serde::{self, Deserialize, Serialize};

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
