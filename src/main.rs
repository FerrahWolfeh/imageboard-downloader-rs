use crate::imageboards::danbooru::DanbooruDownloader;
use crate::imageboards::ImageBoards;
use anyhow::Error;
use clap::Parser;
use std::path::PathBuf;

extern crate tokio;

mod imageboards;
mod progress_bars;

#[derive(Parser, Debug)]
#[clap(name = "Imageboard Downloader", author, version, about, long_about = None)]
struct Cli {
    /// Specify imageboard to download from
    #[clap(short, long, arg_enum, ignore_case = true, default_value_t = ImageBoards::Danbooru)]
    imageboard: ImageBoards,

    /// Output dir
    #[clap(short, parse(from_os_str))]
    output: Option<PathBuf>,

    /// Tags to search
    #[clap(value_parser, required = true)]
    tags: Vec<String>,

    /// Number of simultaneous downloads
    #[clap(short, value_parser, default_value_t = 3)]
    simultaneous_downloads: usize,

    /// Download images from safe version of imageboard
    #[clap(long, action, default_value_t = false)]
    safe_mode: bool,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Cli::parse();
    env_logger::builder().format_timestamp(None).init();

    match args.imageboard {
        ImageBoards::Danbooru => {
            if let Ok(mut dl) = DanbooruDownloader::new(
                &args.tags,
                args.output,
                args.simultaneous_downloads,
                args.safe_mode,
            )
            .await
            {
                dl.download().await?;
            } else {
                println!("No posts found for tag selection!")
            }
        }
        ImageBoards::E621 => todo!(),
        ImageBoards::Rule34 => todo!(),
        ImageBoards::Realbooru => todo!(),
    };

    Ok(())
}
