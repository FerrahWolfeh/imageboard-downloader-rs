use crate::imageboards::auth::AuthCredentials;
use crate::imageboards::danbooru::DanbooruDownloader;
use crate::imageboards::e621::E621Downloader;
use crate::imageboards::ImageBoards;
use anyhow::Error;
use clap::Parser;
use colored::Colorize;
use log::debug;
use std::io;
use std::io::Write;
use std::path::PathBuf;

extern crate tokio;

mod imageboards;
mod progress_bars;

#[derive(Parser, Debug)]
#[clap(name = "Imageboard Downloader", author, version, about, long_about = None)]
struct Cli {
    /// Tags to search
    #[clap(value_parser, required = true)]
    tags: Vec<String>,

    /// Specify which website to download from
    #[clap(short, long, arg_enum, ignore_case = true, default_value_t = ImageBoards::Danbooru)]
    imageboard: ImageBoards,

    /// Where to save downloaded files
    #[clap(short, long, parse(from_os_str), value_name = "PATH")]
    output: Option<PathBuf>,

    /// Number of simultaneous downloads
    #[clap(
        short = 'd',
        value_name = "NUMBER",
        value_parser,
        default_value_t = 3,
        help_heading = "GENERAL"
    )]
    simultaneous_downloads: usize,

    /// Authenticate to the imageboard website.
    ///
    /// This flag only needs to be set a single time.
    #[clap(short, long, action)]
    auth: bool,

    /// Download images from the safe version of the selected Imageboard.
    ///
    /// Currently only works with Danbooru, e621 and Konachan. This flag will be silently ignored if other imageboard is selected
    ///
    /// Useful if you only want to download posts with "safe" rating.
    #[clap(
        long,
        action,
        default_value_t = false,
        help_heading = "Danbooru Specific Options"
    )]
    safe_mode: bool,

    /// Save files with their ID as filename instead of it's MD5
    #[clap(long = "id", value_parser, default_value_t = false)]
    save_file_as_id: bool,
}

async fn try_auth(auth_state: bool, imageboard: ImageBoards) -> Result<(), Error> {
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

        let at = AuthCredentials::new(username.trim().to_string(), api_key.trim().to_string());

        at.authenticate(imageboard).await?;

        return Ok(());
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Cli::parse();
    env_logger::builder().format_timestamp(None).init();

    match args.imageboard {
        ImageBoards::Danbooru => {
            try_auth(args.auth, args.imageboard).await?;
            let mut dl = DanbooruDownloader::new(
                &args.tags,
                args.output,
                args.simultaneous_downloads,
                args.safe_mode,
                args.save_file_as_id,
            )?;

            dl.download().await?;
        }
        ImageBoards::E621 => {
            try_auth(args.auth, args.imageboard).await?;
            let mut dl = E621Downloader::new(
                &args.tags,
                args.output,
                args.simultaneous_downloads,
                args.safe_mode,
                args.save_file_as_id,
            )?;

            dl.download().await?;
        }
        ImageBoards::Rule34 | ImageBoards::Realbooru | ImageBoards::Konachan => todo!(),
    };

    Ok(())
}
