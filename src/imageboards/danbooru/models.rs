use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Serialize, Deserialize, Debug)]
pub struct DanbooruItem {
    pub id: u64,
    pub md5: String,
    pub file_ext: String,
    pub file_url: String,
    pub tags: HashSet<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Count {
    pub posts: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DanbooruAuthUser {
    pub success: Option<bool>,
    pub message: Option<String>,
    pub name: Option<String>,
    pub id: Option<u64>,
    pub blacklisted_tags: Option<String>,
}
