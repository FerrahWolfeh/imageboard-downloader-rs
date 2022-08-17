use anyhow::Error;
use bincode::deserialize;
use log::debug;
use tokio::fs::read;
use crate::ImageBoards;
use crate::imageboards::danbooru::models::DanbooruAuthUser;

impl DanbooruAuthUser {
    pub async fn read_from_fs(imageboard: ImageBoards) -> Result<Option<Self>, Error> {
        if let Ok(config_auth) = read(imageboard.auth_cache_dir()?).await {
            debug!("Authentication cache found");

            return if let Ok(rd) = deserialize::<Self>(&config_auth) {
                debug!("Authentication cache decoded.");
                Ok(Some(rd))
            } else {
                debug!("Authentication cache is invalid or empty. Using normal mode");
                Ok(None)
            };
        };
        debug!("Running without authentication");
        Ok(None)
    }
}