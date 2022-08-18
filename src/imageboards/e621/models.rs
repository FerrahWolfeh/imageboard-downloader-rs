use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct E621TopLevel {
    pub posts: Vec<E621Post>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct E621Post {
    pub id: Option<u64>,
    pub file: E621File,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct E621File {
    pub ext: Option<String>,
    pub md5: Option<String>,
    pub url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct E621AuthUser {
    pub success: Option<bool>,
    pub message: Option<String>,
    pub id: Option<u64>,
    pub name: Option<String>,
    pub blacklisted_tags: Option<String>,
}
