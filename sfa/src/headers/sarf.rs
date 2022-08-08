use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct SARFItemMetadata {
    pub file: String,
    pub file_size: u64,
    pub height: u64,
    pub width: u64,
    pub md5: String,
    pub sha256: String,
}

#[derive(Serialize, Deserialize)]
pub struct SARF {
    pub item_count: usize,
    pub item_list: Vec<SARFItemMetadata>,
}