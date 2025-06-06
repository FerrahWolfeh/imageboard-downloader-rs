use std::{
    collections::HashMap,
    env, io,
    path::{Path, PathBuf},
};

use crate::error::CliError;
use bincode::deserialize;
use dialoguer::{Input, Password, theme::ColorfulTheme};
use ibdl_common::{
    ImageBoards,
    directories::ProjectDirs, // Keep for get_servers
};
use ibdl_extractors::{
    auth::ImageboardConfig,
    blacklist::{DEFAULT_BLACKLIST_TOML, GlobalBlacklist},
    extractor::{PostExtractor, SiteApi},
    extractor_config::{DEFAULT_SERVERS, ServerConfig, serialize::read_server_cfg_file},
};
use log::{debug, info, warn};
use owo_colors::OwoColorize;
use std::fs;
use tokio::{
    fs::{File, create_dir_all, read, read_to_string, remove_file, write},
    io::AsyncWriteExt,
};

use super::AVAILABLE_SERVERS;
use ibdl_extractors::prelude::Auth; // Keep this for the Auth trait

// The `auth_prompt` function is removed as its logic is now integrated into `auth_imgboard`.

/// Handles authentication for an imageboard extractor.
///
/// It first tries to load existing credentials from the cache.
/// If credentials are not found in the cache and `prompt_for_auth` is true,
/// it will prompt the user for their username and API key,
/// attempt to authenticate, and if successful, save the credentials to the cache.
///
/// # Arguments
/// * `prompt_for_auth`: If true, the function will prompt for credentials if none are cached.
///   If false, it will only attempt to use cached credentials.
/// * `extractor`: A mutable reference to an extractor that implements `Auth` and `Extractor`.
///
/// # Errors
/// Returns a `CliError` if any step (cache reading, user input, authentication, cache writing,
/// or applying auth to extractor) fails.
pub async fn auth_imgboard<S: SiteApi + 'static>(
    prompt_for_auth: bool,
    extractor: &mut PostExtractor<S>,
) -> Result<(), CliError> {
    // The old `E: Extractor` constraint is no longer needed as PostExtractor<S>
    // provides `config()` and `client()` methods directly.
    let imageboard_server_config = extractor.config(); // This is &ServerConfig
    let client = extractor.client();

    // Try to load credentials from cache first
    match read_config_from_fs(&imageboard_server_config).await {
        Ok(Some(cached_creds)) => {
            debug!(
                "Using cached credentials for {}",
                imageboard_server_config.name
            );
            // Assuming CliError can convert from ExtractorError
            extractor.auth(cached_creds).await?;
            return Ok(());
        }
        Ok(None) => {
            debug!(
                "No cached credentials found for {}.",
                imageboard_server_config.name
            );
            // Proceed to prompt if allowed
        }
        Err(e) => {
            warn!(
                "Failed to read auth cache for {}: {}. Proceeding without cached auth.",
                imageboard_server_config.name, e
            );
            // Proceed to prompt if allowed, otherwise run unauthenticated.
            // The error 'e' here is an io::Error. If it's critical, it should be returned.
            // For now, we log and try to prompt if allowed.
        }
    }

    // If cache not found/invalid AND prompt_for_auth is true, then prompt the user
    if prompt_for_auth {
        info!(
            "{} {}",
            "Attempting to log into:".bold(),
            imageboard_server_config.name.green().bold()
        );

        let username: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Username")
            .interact()?;

        let api_key: String = Password::with_theme(&ColorfulTheme::default())
            .with_prompt("API Key")
            .interact()?;

        let mut fresh_config = ImageboardConfig::new(
            imageboard_server_config.clone(), // Clone the ServerConfig from extractor
            username.trim().to_string(),
            api_key.trim().to_string(),
        );

        fresh_config.authenticate(&client).await?;

        info!(
            "Successfully authenticated as user: {}",
            fresh_config.user_data.name
        );

        // Serialize and write the new config to cache
        let bytes = fresh_config.to_bincode_bytes()?;
        let pretty_name = fresh_config.server_pretty_name();
        let cache_dir = ImageBoards::auth_cache_dir()?;

        if !cache_dir.exists() {
            create_dir_all(&cache_dir).await?;
        }

        let config_file_path = cache_dir.join(pretty_name);
        write(&config_file_path, &bytes).await?;
        info!(
            "Auth cache for {} saved to {}",
            pretty_name,
            config_file_path.display()
        );

        // Use the freshly authenticated and cached config for the extractor
        extractor.auth(fresh_config).await?;
    } else {
        debug!(
            "Not prompting for auth and no cache found for {}. Running unauthenticated.",
            imageboard_server_config.name
        );
    }

    Ok(())
}

/// Reads and parses the authentication cache from the path provided by `auth_cache_dir`.
///
/// Returns `None` if the file is corrupted or does not exist.
pub async fn read_config_from_fs(
    imageboard: &ServerConfig,
) -> Result<Option<ImageboardConfig>, io::Error> {
    // ImageBoards::auth_cache_dir() returns Result<PathBuf, ibdl_common::Error>
    // Map the error to io::Error to match this function's signature.
    let cache_dir = ImageBoards::auth_cache_dir()?;
    let cfg_path = cache_dir.join(PathBuf::from(imageboard.pretty_name.clone()));
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
                "Auth cache for {} is invalid or empty. Removing.",
                imageboard.name
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

pub async fn init_blacklist() -> Result<GlobalBlacklist, CliError> {
    // Load global blacklist configuration
    let proj_dirs = ProjectDirs::from("com", "ferrahwolfeh", "imageboard-downloader").unwrap();

    let config_dir = proj_dirs.config_dir();
    if !config_dir.exists() {
        create_dir_all(config_dir).await?;
    }

    let blacklist_path = config_dir.join("blacklist.toml");

    let blacklist_content = if blacklist_path.exists() {
        read_to_string(&blacklist_path).await?
    } else {
        let default_content = DEFAULT_BLACKLIST_TOML.to_string();
        let mut file = File::create(&blacklist_path).await?;
        file.write_all(default_content.as_bytes()).await?;
        default_content
    };

    let global_blacklist = GlobalBlacklist::from_config(&blacklist_content)?;

    Ok(global_blacklist)
}
