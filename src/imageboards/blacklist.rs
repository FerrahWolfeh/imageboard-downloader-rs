use anyhow::Error;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GlobalBlacklist {
    /// In this array, the user will declare tags that should be excluded from all imageboards
    global_blacklist: Vec<String>,
}

impl GlobalBlacklist {
    pub fn get_gbl() -> Result<Self, Error> {
        todo!()
    }

    fn path() -> PathBuf {
        todo!()
    }
}
