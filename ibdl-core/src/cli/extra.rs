use std::{
    collections::HashMap,
    env, io,
    path::{Path, PathBuf},
};

use crate::error::CliError;
use dialoguer::{theme::ColorfulTheme, Input, Password};
use ibdl_common::{
    bincode::deserialize,
    directories::ProjectDirs,
    log::{debug, warn},
    reqwest::Client,
    tokio::fs::{read, remove_file},
    ImageBoards,
};
use ibdl_extractors::prelude::{Auth, Extractor};
use ibdl_extractors::{
    auth::ImageboardConfig,
    extractor_config::{serialize::read_server_cfg_file, ServerConfig, DEFAULT_SERVERS},
};
use owo_colors::OwoColorize;
use std::fs;

use super::AVAILABLE_SERVERS;

pub async fn auth_prompt(
    auth_state: bool,
    imageboard: &ServerConfig,
    client: &Client,
) -> Result<(), CliError> {
    if auth_state {
        println!(
            "{} {}",
            "Logging into:".bold(),
            imageboard.to_string().green().bold()
        );

        let username: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Username")
            .interact()?;

        let api_key: String = Password::with_theme(&ColorfulTheme::default())
            .with_prompt("API Key")
            .interact()?;

        let mut at = ImageboardConfig::new(
            get_servers().get(&imageboard.name).unwrap().clone(),
            username.trim().to_string(),
            api_key.trim().to_string(),
        );

        at.authenticate(client).await?;

        return Ok(());
    }
    Ok(())
}

pub async fn auth_imgboard<E>(ask: bool, extractor: &mut E) -> Result<(), CliError>
where
    E: Auth + Extractor + Send,
{
    let imageboard = extractor.config();
    let client = extractor.client();
    auth_prompt(ask, &imageboard, &client).await?;

    if let Some(creds) = read_config_from_fs(&imageboard).await? {
        extractor.auth(creds).await?;
        return Ok(());
    }

    Ok(())
}

/// Reads and parses the authentication cache from the path provided by `auth_cache_dir`.
///
/// Returns `None` if the file is corrupted or does not exist.
pub async fn read_config_from_fs(
    imageboard: &ServerConfig,
) -> Result<Option<ImageboardConfig>, io::Error> {
    let cfg_path = ImageBoards::auth_cache_dir()?.join(PathBuf::from(imageboard.to_string()));
    if let Ok(config_auth) = read(&cfg_path).await {
        debug!("Authentication cache found");

        if let Ok(rd) = deserialize::<ImageboardConfig>(&config_auth) {
            debug!("Authentication cache decoded.");
            debug!("User id: {}", rd.user_data.id);
            debug!("Username: {}", rd.user_data.name);
            debug!("Blacklisted tags: '{:?}'", rd.user_data.blacklisted_tags);
            return Ok(Some(rd));
        } else {
            warn!(
                "{}",
                "Auth cache is invalid or empty. Running without authentication"
            );
            debug!("Removing corrupted file");
            remove_file(cfg_path).await?;
            return Ok(None);
        };
    };
    debug!("Running without authentication");
    Ok(None)
}

pub fn get_servers<'a>() -> &'a HashMap<String, ServerConfig> {
    AVAILABLE_SERVERS.get_or_init(|| {
        let mut servers = DEFAULT_SERVERS.clone();

        let cfg_path = PathBuf::from(env::var("IBDL_SERVER_CFG").unwrap_or_else(|_| {
            let cdir = ProjectDirs::from("com", "FerrahWolfeh", "imageboard-downloader").unwrap();
            cdir.config_dir().to_string_lossy().to_string()
        }));

        if !cfg_path.exists() {
            fs::create_dir_all(&cfg_path).unwrap();
        }

        let cfg_path = cfg_path.join(Path::new("servers.toml"));

        read_server_cfg_file(&cfg_path, &mut servers);

        servers
    })
}

pub fn validate_imageboard(input: &str) -> Result<ServerConfig, String> {
    let servers = get_servers();

    servers.get(input).map_or_else(
        || {
            Err(format!(
                "Invalid imageboard: {}. Allowed imageboards are: {:?}",
                input,
                servers.keys()
            ))
        },
        |server| Ok(server.clone()),
    )
}
