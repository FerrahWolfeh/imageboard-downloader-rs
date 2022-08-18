use crate::imageboards::auth::ImageboardConfig;
use crate::imageboards::danbooru::DanbooruDownloader;
use crate::imageboards::e621::E621Downloader;
use crate::imageboards::konachan::KonachanDownloader;
use crate::imageboards::rule34::R34Downloader;
use crate::imageboards::ImageBoards;
use anyhow::Error;
use clap::Parser;

use std::path::PathBuf;
use crate::imageboards::realbooru::RealbooruDownloader;

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
    #[clap(
        short,
        long,
        parse(from_os_str),
        value_name = "PATH",
        help_heading = "DOWNLOAD"
    )]
    output: Option<PathBuf>,

    /// Number of simultaneous downloads
    #[clap(
        short = 'd',
        value_name = "NUMBER",
        value_parser,
        default_value_t = 3,
        help_heading = "DOWNLOAD"
    )]
    simultaneous_downloads: usize,

    /// Authenticate to the imageboard website.
    ///
    /// This flag only needs to be set a single time.
    ///
    /// Once authenticated, it's possible to use your blacklist to exclude posts with unwanted tags
    #[clap(short, long, action, help_heading = "GENERAL")]
    auth: bool,

    /// Download images from the safe version of the selected Imageboard.
    ///
    /// Currently only works with Danbooru, e621 and Konachan. This flag will be silently ignored if other imageboard is selected
    ///
    /// Useful if you only want to download posts with "safe" rating.
    #[clap(long, action, default_value_t = false, help_heading = "GENERAL")]
    safe_mode: bool,

    /// Save files with their ID as filename instead of it's MD5
    #[clap(
        long = "id",
        value_parser,
        default_value_t = false,
        help_heading = "DOWNLOAD"
    )]
    save_file_as_id: bool,
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
                args.auth,
                args.save_file_as_id,
            )
            .await?;

            dl.download().await?;
        }
        ImageBoards::E621 => {
            let mut dl = E621Downloader::new(
                &args.tags,
                args.output,
                args.simultaneous_downloads,
                args.auth,
                args.safe_mode,
                args.save_file_as_id,
            )
            .await?;

            dl.download().await?;
        }
        ImageBoards::Rule34 => {
            let mut dl = R34Downloader::new(
                &args.tags,
                args.output,
                args.simultaneous_downloads,
                args.save_file_as_id,
            )?;

            dl.download().await?;
        }
        ImageBoards::Konachan => {
            let mut dl = KonachanDownloader::new(
                &args.tags,
                args.output,
                args.simultaneous_downloads,
                args.safe_mode,
                args.save_file_as_id,
            )?;

            dl.download().await?;
        }
        ImageBoards::Realbooru => {
            let mut dl = RealbooruDownloader::new(
                &args.tags,
                args.output,
                args.simultaneous_downloads,
                args.save_file_as_id,
            )?;

            dl.download().await?;
        }
    };

    Ok(())
}
