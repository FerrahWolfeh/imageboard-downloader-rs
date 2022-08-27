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
        io::stdout().flush().unwrap();
        stdin.read_line(&mut username).unwrap();
        print!("{}", "API Key: ".bold());
        io::stdout().flush().unwrap();
        stdin.read_line(&mut api_key).unwrap();

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
