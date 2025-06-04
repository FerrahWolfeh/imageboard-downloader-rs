use ahash::HashMap;
use ibdl_common::{
    log::{debug, trace},
    serde_json,
};

use super::{models::DanbooruPoolList, DanbooruExtractor};
use crate::error::ExtractorError;
use crate::extractor::caps::PoolExtract;

impl PoolExtract for DanbooruExtractor {
    async fn fetch_pool_idxs(
        &mut self,
        pool_id: u32,
        limit: Option<u16>,
    ) -> Result<HashMap<u64, usize>, ExtractorError> {
        if self.server_cfg.pool_idx_url.is_none() {
            return Err(ExtractorError::UnsupportedOperation);
        }

        let url = format!(
            "{}/{}.json",
            self.server_cfg.pool_idx_url.as_ref().unwrap(),
            pool_id
        );

        // Fetch item list from page
        let req = if self.auth_state.is_auth() {
            debug!("[AUTH] Fetching post ids from pool {}", pool_id);
            self.client
                .get(url)
                .basic_auth(&self.auth.username, Some(&self.auth.api_key))
        } else {
            debug!("Fetching post ids from pool {}", pool_id);
            self.client.get(url)
        };

        let post_array = req.send().await?.text().await?;

        let mut mtx = self.parse_pool_ids(post_array)?;

        if self.pool_last_items_first {
            mtx.reverse();
        }

        if let Some(limit_post) = limit {
            mtx.truncate(limit_post as usize);
        }

        let position_map = mtx
            .iter()
            .enumerate()
            .map(|(position, id)| (*id, position))
            .collect::<HashMap<u64, usize>>();

        trace!("Pool post positions: {:#?}", position_map);
        debug!("Pool size: {}", position_map.len());
        Ok(position_map)
    }

    fn parse_pool_ids(&self, raw_json: String) -> Result<Vec<u64>, ExtractorError> {
        let parsed_json: DanbooruPoolList =
            serde_json::from_str::<DanbooruPoolList>(raw_json.as_str())?;

        Ok(parsed_json.post_ids)
    }

    fn setup_pool_download(&mut self, pool_id: Option<u32>, last_first: bool) {
        self.pool_id = pool_id;
        self.pool_last_items_first = last_first;
    }
}
