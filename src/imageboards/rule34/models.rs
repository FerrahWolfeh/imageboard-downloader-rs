use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct R34Post {
    pub file_url: Option<String>,
    pub hash: Option<String>,
    pub id: Option<u64>,
    pub image: Option<String>,
}
