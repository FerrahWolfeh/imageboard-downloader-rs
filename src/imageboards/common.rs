//! Common functions for all imageboard downloader modules.
use crate::imageboards::auth::ImageboardConfig;
use crate::imageboards::ImageBoards;
use anyhow::Error;
use colored::Colorize;
use log::debug;
use reqwest::Client;
use std::io;
use std::io::Write;
use std::sync::{Arc, Mutex};

/// Checks if ```output_dir``` is set in cli args then returns a ```PathBuf``` pointing to where the files will be downloaded.
///
/// In case the user does not set a value with the ```-o``` flag, the function will default to the current dir where the program is running.
///
/// The path chosen will always end with the imageboard name followed by the tags used.
///
/// ```rust
///
/// use std::path::PathBuf;
/// use imageboard_downloader::imageboards::ImageBoards;
/// use imageboard_downloader::join_tags;
///
/// let tags = join_tags!(["kroos_(arknights)", "weapon"]);
/// let path = Some(PathBuf::from("./"));
///
/// let out_dir = generate_out_dir(path, &tags, ImageBoards::Danbooru).unwrap();
///
/// assert_eq!(PathBuf::from("./danbooru/kroos_(arknights) weapon"), out_dir);
/// ```

/// Struct to condense both counters that are used when downloading and checking limits
#[derive(Clone)]
pub struct Counters {
    pub total_mtx: Arc<Mutex<usize>>,
    pub downloaded_mtx: Arc<Mutex<u64>>,
}

pub async fn auth_prompt(
    auth_state: bool,
    imageboard: ImageBoards,
    client: &Client,
) -> Result<(), Error> {
    if auth_state {
        let mut username = String::new();
        let mut api_key = String::new();
        let stdin = io::stdin();
        println!(
            "{} {}",
            "Logging into:".bold(),
            imageboard.to_string().green().bold()
        );
        print!("{}", "Username: ".bold());
        io::stdout().flush()?;
        stdin.read_line(&mut username)?;
        print!("{}", "API Key: ".bold());
        io::stdout().flush()?;
        stdin.read_line(&mut api_key)?;

        debug!("Username: {:?}", username.trim());
        debug!("API key: {:?}", api_key.trim());

        let mut at = ImageboardConfig::new(
            imageboard,
            username.trim().to_string(),
            api_key.trim().to_string(),
        );

        at.authenticate(client).await?;

        return Ok(());
    }
    Ok(())
}
