use std::{io, io::Write};

use color_eyre::eyre::Result;
use ibdl_common::{auth::ImageboardConfig, log::debug, reqwest::Client, ImageBoards};
use ibdl_core::owo_colors::OwoColorize;
use ibdl_extractors::websites::{Auth, Extractor};

pub fn print_results(total_down: u64, total_black: u64) {
    println!(
        "{} {} {}",
        total_down.to_string().bold().blue(),
        "files".bold().blue(),
        "downloaded".bold()
    );

    if total_black > 0 && total_down != 0 {
        println!(
            "{} {}",
            total_black.to_string().bold().red(),
            "posts with blacklisted tags were not downloaded."
                .bold()
                .red()
        );
    }
}

pub async fn auth_prompt(auth_state: bool, imageboard: ImageBoards, client: &Client) -> Result<()> {
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

        debug!("Username: {}", username.trim());
        debug!("API key: {}", api_key.trim());

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

pub async fn auth_imgboard<E>(ask: bool, extractor: &mut E) -> Result<()>
where
    E: Auth + Extractor,
{
    let imageboard = extractor.imageboard();
    let client = extractor.client();
    auth_prompt(ask, imageboard, &client).await?;

    if let Some(creds) = imageboard.read_config_from_fs().await? {
        extractor.auth(creds).await?;
        return Ok(());
    }

    Ok(())
}
