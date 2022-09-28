use ibdl_common::serde::{self, Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "self::serde")]
pub struct DanbooruPost {
    pub id: Option<u64>,
    pub md5: Option<String>,
    pub file_url: Option<String>,
    pub tag_string: Option<String>,
    pub file_ext: Option<String>,
    pub rating: Option<String>,
}
