use ahash::AHashSet;
use anyhow::Error;
use log::debug;
use serde::{Deserialize, Serialize};
use tokio::fs::{read_to_string, File};
use tokio::io::AsyncWriteExt;
use toml::from_str;
use xdg::BaseDirectories;

const BF_INIT_TEXT: &[u8; 92] = b"# Place in the array all the tags that will be excluded from all imageboards\n\nblacklist = []";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GlobalBlacklist {
    /// In this array, the user will declare tags that should be excluded from all imageboards
    pub blacklist: Option<AHashSet<String>>,
}

impl GlobalBlacklist {
    pub async fn get() -> Result<Self, Error> {
        let xdg_dir = BaseDirectories::with_prefix("imageboard-downloader")?;

        let dir = xdg_dir.place_config_file("blacklist.toml")?;

        if !dir.exists() {
            debug!("Creating blacklist file");
            File::create(&dir).await?.write_all(BF_INIT_TEXT).await?;
        }

        let gbl_string = read_to_string(&dir).await?;
        let deserialized = from_str::<Self>(&gbl_string)?;
        debug!("Global blacklist decoded");
        Ok(deserialized)
    }
}
