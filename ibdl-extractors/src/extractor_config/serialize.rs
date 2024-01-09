use ibdl_common::{
    log::debug,
    serde::{self, Deserialize},
    ImageBoards,
};
use std::{collections::HashMap, fs::read_to_string, io::Write, str::FromStr};
use std::{fs::File, path::Path};
use toml;

use crate::extractor_config::{DEFAULT_CLI_UA, DEFAULT_EXT_UA};

use super::ServerConfig;

const SAMPLE_SERVER_TOML: &str = include_str!("sample.toml");

#[derive(Debug, Deserialize)]
#[serde(crate = "self::serde")]
struct Config {
    servers: HashMap<String, Server>,
}

#[derive(Debug, Deserialize)]
#[serde(crate = "self::serde")]
struct Server {
    pretty_name: String,
    server: String,
    base_url: String,
    post_url: Option<String>,
    post_list_url: Option<String>,
    pool_idx_url: Option<String>,
    max_post_limit: usize,
    auth_url: Option<String>,
    image_url: Option<String>,
}

pub fn read_server_cfg_file(path: &Path, smap: &mut HashMap<String, ServerConfig>) {
    if !path.exists() {
        let mut sample_toml = File::create(path).unwrap();
        sample_toml
            .write_all(SAMPLE_SERVER_TOML.as_bytes())
            .unwrap();
    }

    let contents = read_to_string(path).expect("Something went wrong reading the file");

    let config: Config = toml::from_str(&contents).unwrap();

    for (id, data) in config.servers {
        let config = ServerConfig {
            name: id.clone(),
            pretty_name: data.pretty_name,
            server: ImageBoards::from_str(&data.server).unwrap(),
            client_user_agent: DEFAULT_CLI_UA.to_string(),
            extractor_user_agent: DEFAULT_EXT_UA.to_string(),
            base_url: data.base_url,
            post_url: data.post_url,
            post_list_url: data.post_list_url,
            pool_idx_url: data.pool_idx_url,
            max_post_limit: data.max_post_limit,
            auth_url: data.auth_url,
            image_url: data.image_url,
        };
        smap.insert(id, config);
    }

    debug!("Configured servers: {:?}", smap)
}
