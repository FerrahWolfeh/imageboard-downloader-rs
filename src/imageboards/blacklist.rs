use ahash::AHashSet;
use anyhow::Error;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs::read_to_string;
use toml::from_str;
use xdg::BaseDirectories;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GlobalBlacklist {
    /// In this array, the user will declare tags that should be excluded from all imageboards
    pub global_blacklist: Option<AHashSet<String>>,
}

impl GlobalBlacklist {
    pub async fn get() -> Result<Self, Error> {
        let gbl_string = read_to_string(Self::path()?).await?;
        let deserialized = from_str::<Self>(&gbl_string)?;
        Ok(deserialized)
    }

    fn path() -> Result<PathBuf, Error> {
        let xdg_dir = BaseDirectories::with_prefix("imageboard-downloader")?;

        let dir = xdg_dir.place_config_file("blacklist.toml")?;
        Ok(dir)
    }
}
