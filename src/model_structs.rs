use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct DanbooruPost {
    pub file_size: u64,
    pub md5: Option<String>,
    pub file_ext: Option<String>,
    pub file_url: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Count {
    pub posts: f32,
}

#[derive(Serialize, Deserialize)]
pub struct DanbooruPostCount {
    pub counts: Count,
}

#[derive(Serialize, Deserialize)]
pub struct SAFItemMetadata {
    pub file: String,
    pub file_size: u64,
    pub md5: String,
    pub sha256: String,
}

#[derive(Serialize, Deserialize)]
pub struct SAFMetadata {
    pub item_count: usize,
    pub item_list: Vec<SAFItemMetadata>,
}
