use anyhow::Error;
use crate::ImageBoards;

pub struct AuthCredentials {
    username: String,
    api_key: String,
}

impl AuthCredentials {
    pub fn new(username: String, api_key: String) -> Self {
        Self {
            username,
            api_key,
        }
    }

    pub async fn authenticate(&self, imageboard: ImageBoards) -> Result<(), Error> {
        todo!()
    }
}