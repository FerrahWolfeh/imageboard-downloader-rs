use crate::imageboards::danbooru::models::Auth;
use crate::ImageBoards;
use anyhow::{bail, Error};
use bincode::{deserialize, serialize};
use log::debug;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::fs::{read, OpenOptions};
use tokio::io::AsyncWriteExt;

#[derive(Serialize, Deserialize)]
pub struct AuthCredentials {
    pub username: String,
    pub api_key: String,
}

impl AuthCredentials {
    pub fn new(username: String, api_key: String) -> Self {
        Self { username, api_key }
    }

    pub async fn read_from_fs(imageboard: ImageBoards) -> Result<Option<Self>, Error> {
        if let Ok(config_auth) = read(imageboard.auth_cache_dir()?).await {
            debug!("Authentication cache found");

            return if let Ok(rd) = deserialize::<AuthCredentials>(&config_auth) {
                debug!("Authentication cache decoded. Using authentication");
                Ok(Some(Self {
                    username: rd.username,
                    api_key: rd.api_key,
                }))
            } else {
                debug!("Authentication cache is empty. Using normal mode");
                Ok(None)
            };
        };
        debug!("Running without authentication");
        Ok(None)
    }

    pub async fn authenticate(&self, imageboard: ImageBoards) -> Result<(), Error> {
        match imageboard {
            ImageBoards::Danbooru => Self::danbooru_auth(self).await,
            _ => todo!(),
        }
    }

    async fn danbooru_auth(&self) -> Result<(), Error> {
        let imageboard = ImageBoards::Danbooru;
        let client = Client::builder()
            .user_agent(imageboard.user_agent())
            .build()?;
        let req = client
            .get(imageboard.auth_url())
            .basic_auth(&self.username, Some(&self.api_key))
            .send()
            .await?
            .json::<Auth>()
            .await?;

        debug!("{:?}", req);

        if req.success.is_some() {
            bail!("Invalid username or api key!")
        }

        if req.id.is_some() {
            let config_path = imageboard.auth_cache_dir()?;
            let mut cfg_cache = OpenOptions::new()
                .create(true)
                .truncate(true)
                .read(true)
                .write(true)
                .open(config_path)
                .await?;
            let cfg = serialize(&self)?;
            cfg_cache.write_all(&cfg).await?;
        }

        Ok(())
    }
}
