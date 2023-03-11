use std::{io, io::Write, path::PathBuf};

use color_eyre::eyre::Result;
use ibdl_common::{
    auth::ImageboardConfig, log::debug, post::rating::Rating, reqwest::Client, ImageBoards,
};
use ibdl_core::{
    cli::Cli, generate_output_path, generate_output_path_precise, owo_colors::OwoColorize,
};
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

pub fn generate_save_path(args: &Cli) -> Result<PathBuf> {
    let raw_save_path = if let Some(path) = &args.output {
        path.to_owned()
    } else if let Some(precise_path) = &args.precise_output {
        precise_path.to_owned()
    } else {
        std::env::current_dir()?
    };

    let dirname = if args.output.is_some() {
        assert_eq!(args.precise_output, None);
        generate_output_path(&raw_save_path, *args.imageboard, &args.tags, args.cbz)
    } else if args.precise_output.is_some() {
        assert_eq!(args.output, None);
        generate_output_path_precise(&raw_save_path, args.cbz)
    } else {
        raw_save_path
    };

    Ok(dirname)
}

#[inline]
pub fn convert_rating_list(args: &Cli) -> Vec<Rating> {
    let mut ratings: Vec<Rating> = Vec::with_capacity(4);
    if args.rating.is_empty() {
        if args.safe_mode {
            ratings.push(Rating::Safe);
        } else {
            ratings.push(Rating::Safe);
            ratings.push(Rating::Questionable);
            ratings.push(Rating::Explicit)
        }
    } else {
        args.rating.iter().for_each(|item| ratings.push(item.0));
    };

    if !args.ignore_unknown {
        ratings.push(Rating::Unknown);
    }
    ratings
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
