use async_trait::async_trait;
use ibdl_common::{log::debug, serde_json, ImageBoards};

use crate::{error::ExtractorError, websites::PoolExtract};

use super::{models::E621PoolList, E621Extractor};

#[async_trait]
impl PoolExtract for E621Extractor {
    async fn fetch_pool_idxs(&mut self, pool_id: u32) -> Result<Vec<u64>, ExtractorError> {
        let url = format!("{}/{}.json", ImageBoards::E621.pool_idx_url(), pool_id);

        // Fetch item list from page
        let req = if self.auth_state {
            debug!("[AUTH] Fetching post ids from pool {}", pool_id);
            self.client
                .get(url)
                .basic_auth(&self.auth.username, Some(&self.auth.api_key))
        } else {
            debug!("Fetching post ids from pool {}", pool_id);
            self.client.get(url)
        };

        let post_array = req.send().await?.text().await?;

        let mtx = self.parse_pool_ids(post_array)?;

        debug!("Pool size: {}", mtx.len());
        Ok(mtx)
    }

    fn parse_pool_ids(&self, raw_json: String) -> Result<Vec<u64>, ExtractorError> {
        let parsed_json: E621PoolList = serde_json::from_str::<E621PoolList>(raw_json.as_str())?;

        Ok(parsed_json.post_ids)
    }

    fn setup_pool_download(&mut self, pool_id: Option<u32>) {
        self.pool_id = pool_id;
    }
}