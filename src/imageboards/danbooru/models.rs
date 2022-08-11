use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct DanbooruItem {
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

#[derive(Serialize, Deserialize, Debug)]
pub struct Auth {
    pub login: String,
    pub api_key: String,
}
