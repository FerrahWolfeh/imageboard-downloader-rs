use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct DanbooruItem {
    pub id: Option<u64>,
    pub md5: Option<String>,
    pub file_ext: Option<String>,
    pub file_url: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Count {
    pub posts: u64,
}

#[derive(Serialize, Deserialize)]
pub struct DanbooruPostCount {
    pub counts: Count,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DanbooruAuthUser {
    pub success: Option<bool>,
    pub message: Option<String>,
    pub name: Option<String>,
    pub id: Option<u64>,
    pub blacklisted_tags: Option<String>,
}