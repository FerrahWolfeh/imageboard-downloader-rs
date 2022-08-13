use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct E621TopLevel {
    pub posts: Vec<E621Post>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct E621Post {
    pub file: E621File,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct E621File {
    pub ext: Option<String>,
    pub md5: Option<String>,
    pub url: Option<String>,
}
