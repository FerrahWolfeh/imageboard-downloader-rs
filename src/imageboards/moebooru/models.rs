use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct KonachanPost {
    pub id: Option<u64>,
    pub md5: Option<String>,
    pub file_url: Option<String>,
    pub rating: String,
    pub tags: String,
}
