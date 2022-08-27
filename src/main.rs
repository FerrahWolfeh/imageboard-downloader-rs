use anyhow::Error;
use cfg_if::cfg_if;
use clap::Parser;
use imageboard_downloader::{imageboards::extractors::ImageBoardExtractor, *};
use log::debug;
use std::path::PathBuf;

extern crate tokio;

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

    /// Limit max number of downloads
    #[clap(short, long, value_parser, help_heading = "DOWNLOAD")]
    limit: Option<usize>,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args: Cli = Cli::parse();
    env_logger::builder().format_timestamp(None).init();

    match args.imageboard {
        ImageBoards::Danbooru => {
            cfg_if! {
                if #[cfg(feature = "danbooru")] {
                    let mut unit = DanbooruDownloader::new(&args.tags, args.safe_mode)?;
                    unit.auth(args.auth).await?;
                    let posts = unit.full_search().await?;

                    debug!("Collected {} valid posts", posts.posts.len());

                    let mut qw = Queue::new(
                        args.imageboard,
                        posts,
                        args.simultaneous_downloads,
                        args.limit,
                        unit.auth.user_data.blacklisted_tags,
                    );

                    qw.download(args.output, args.save_file_as_id).await?;
                } else {
                    println!("This build does not include support for this imageboard")
                }
            }
        }
        ImageBoards::E621 => {
            cfg_if! {
                if #[cfg(feature = "e621")] {
                    let mut dl = E621Downloader::new(
                        &args.tags,
                        args.output,
                        args.simultaneous_downloads,
                        args.limit,
                        args.auth,
                        args.safe_mode,
                        args.save_file_as_id,
                    )
                    .await?;

                    dl.download().await?;
                } else {
                    println!("This build does not include support for this imageboard")
                }
            }
        }
        ImageBoards::Rule34 | ImageBoards::Realbooru | ImageBoards::Gelbooru => {
            cfg_if! {
                if #[cfg(feature = "gelbooru")] {
                    let mut dl = GelbooruDownloader::new(
                        args.imageboard,
                        &args.tags,
                        args.output,
                        args.simultaneous_downloads,
                        args.limit,
                        args.save_file_as_id,
                    )?;

                    dl.download().await?;
                } else {
                    println!("This build does not include support for this imageboard")
                }
            }
        }
        ImageBoards::Konachan => {
            cfg_if! {
                if #[cfg(feature = "moebooru")] {
                    let mut dl = MoebooruDownloader::new(
                        &args.tags,
                        args.output,
                        args.simultaneous_downloads,
                        args.limit,
                        args.safe_mode,
                        args.save_file_as_id,
                    )?;

                    dl.download().await?;
                } else {
                    println!("This build does not include support for this imageboard")
                }
            }
        }
    };

    Ok(())
}
