use crate::imageboards::danbooru::models::DanbooruAuthUser;
use crate::ImageBoards;
use anyhow::{bail, Error};
use bincode::{deserialize, serialize};
use log::debug;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use tokio::fs::{read, OpenOptions};
use tokio::io::AsyncWriteExt;
use crate::imageboards::common::write_cache;
use crate::imageboards::e621::models::E621AuthUser;

#[derive(Serialize, Deserialize)]
pub struct AuthCredentials {
    pub username: String,
    pub api_key: String,
}

impl AuthCredentials {
    pub fn new(username: String, api_key: String) -> Self {
        Self { username, api_key }
    }

    pub async fn authenticate(&self, imageboard: ImageBoards) -> Result<(), Error> {
        match imageboard {
            ImageBoards::Danbooru => Self::danbooru_auth(self).await,
            ImageBoards::E621 => todo!(),
            _ => todo!(),
        }
    }

    async fn e621_auth(&self) -> Result<(), Error> {
        let imageboard = ImageBoards::E621;
        let client = Client::builder()
            .user_agent(imageboard.user_agent())
            .build()?;
        let req = client
            .get(imageboard.auth_url())
            .basic_auth(&self.username, Some(&self.api_key))
            .send()
            .await?
            .json::<E621AuthUser>()
            .await?;

        debug!("{:?}", req);

        if req.success.is_some() {
            bail!("Invalid username or api key!")
        }

        if req.id.is_some() {
            write_cache(imageboard, req).await?;
        }

        Ok(())
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
            .json::<DanbooruAuthUser>()
            .await?;

        debug!("{:?}", req);

        if req.success.is_some() {
            bail!("Invalid username or api key!")
        }

        if req.id.is_some() {
            write_cache(imageboard, req).await?;
        }

        Ok(())
    }
}
