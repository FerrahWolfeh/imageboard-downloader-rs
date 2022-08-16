use crate::imageboards::danbooru::DanbooruDownloader;
use crate::imageboards::e621::E621Downloader;
use crate::imageboards::ImageBoards;
use anyhow::Error;
use clap::Parser;
use std::path::PathBuf;
use std::io;

extern crate tokio;

mod imageboards;
mod progress_bars;
mod auth;

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
    #[clap(long, action, default_value_t = false, help_heading = "GENERAL")]
    safe_mode: bool,
}

async fn do_auth(auth_state: bool, imageboard: ImageBoards) -> Result<(), Error> {
    if auth_state {
        let mut username = String::new();
        let mut api_key = String::new();
        let stdin = io::stdin();
        println!("Enter your username.");
        stdin.read_line(&mut username)?;
        println!("Enter the imageboard`s API key");
        stdin.read_line(&mut api_key)?;
        return Ok(())
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Cli::parse();
    env_logger::builder().format_timestamp(None).init();

    match args.imageboard {
        ImageBoards::Danbooru => {
            let mut dl = DanbooruDownloader::new(
                &args.tags,
                args.output,
                args.simultaneous_downloads,
                args.safe_mode,
            )?;

            dl.download().await?;
        }
        ImageBoards::E621 => {
            let mut dl = E621Downloader::new(
                &args.tags,
                args.output,
                args.simultaneous_downloads,
                args.safe_mode,
            )?;

            dl.download().await?;
        }
        ImageBoards::Rule34 => todo!(),
        ImageBoards::Realbooru => todo!(),
        ImageBoards::Konachan => todo!(),
    };

    Ok(())
}
