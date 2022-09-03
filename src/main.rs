use anyhow::Error;
use clap::Parser;
use colored::Colorize;
use imageboard_downloader::*;
use log::debug;
use std::{io::Write, path::PathBuf};

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
        help_heading = "SAVE"
    )]
    output: Option<PathBuf>,

    /// Number of simultaneous downloads
    #[clap(
        short = 'd',
        value_name = "NUMBER",
        value_parser(clap::value_parser!(u64).range(1..=20)),
        default_value_t = 3,
        help_heading = "DOWNLOAD"
    )]
    simultaneous_downloads: u64,

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
    ///
    /// If the output dir has the same file downloaded with the MD5 name, it will be renamed to the post's ID
    #[clap(
        long = "id",
        value_parser,
        default_value_t = false,
        help_heading = "SAVE"
    )]
    save_file_as_id: bool,

    /// Limit max number of downloads
    #[clap(short, long, value_parser, help_heading = "DOWNLOAD")]
    limit: Option<usize>,

    /// Ignore both user and global blacklists
    #[clap(long, value_parser, default_value_t = false, help_heading = "GENERAL")]
    disable_blacklist: bool,

    /// Save posts inside a cbz file
    #[clap(long, value_parser, default_value_t = false, help_heading = "SAVE")]
    cbz: bool,

    /// Select from which page to start scanning posts
    #[clap(
        short,
        long,
        value_parser,
        help_heading = "DOWNLOAD",
        value_name = "PAGE"
    )]
    start_page: Option<usize>,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args: Cli = Cli::parse();
    env_logger::builder().format_timestamp(None).init();

    print!(
        "{}{}",
        "Scanning for posts, please wait".bold(),
        "...".bold().blink()
    );
    std::io::stdout().flush()?;

    let (post_queue, total_black, client) = match args.imageboard {
        ImageBoards::Danbooru => {
            let mut unit =
                DanbooruExtractor::new(&args.tags, args.safe_mode, args.disable_blacklist);
            unit.auth(args.auth).await?;
            let posts = unit.full_search(args.start_page, args.limit).await?;

            debug!("Collected {} valid posts", posts.posts.len());

            (posts, unit.total_removed(), unit.client())
        }
        ImageBoards::E621 => {
            let mut unit = E621Extractor::new(&args.tags, args.safe_mode, args.disable_blacklist);
            unit.auth(args.auth).await?;
            let posts = unit.full_search(args.start_page, args.limit).await?;

            debug!("Collected {} valid posts", posts.posts.len());

            (posts, unit.total_removed(), unit.client())
        }
        ImageBoards::Rule34 | ImageBoards::Realbooru | ImageBoards::Gelbooru => {
            let mut unit = GelbooruExtractor::new(&args.tags, false, args.disable_blacklist)
                .set_imageboard(args.imageboard)?;
            let posts = unit.full_search(args.start_page, args.limit).await?;

            debug!("Collected {} valid posts", posts.posts.len());

            (posts, unit.total_removed(), unit.client())
        }
        ImageBoards::Konachan => {
            let mut unit =
                MoebooruExtractor::new(&args.tags, args.safe_mode, args.disable_blacklist);
            let posts = unit.full_search(args.start_page, args.limit).await?;

            debug!("Collected {} valid posts", posts.posts.len());

            (posts, unit.total_removed(), unit.client())
        }
    };

    let mut qw = Queue::new(
        args.imageboard,
        post_queue,
        args.simultaneous_downloads as usize,
        Some(client),
        args.limit,
        args.cbz,
    );

    print!("\r");
    std::io::stdout().flush()?;

    let total_down = qw.download(args.output, args.save_file_as_id).await?;

    println!(
        "{} {} {}",
        total_down.to_string().bold().blue(),
        "files".bold().blue(),
        "downloaded".bold()
    );

    if total_black > 0 {
        println!(
            "{} {}",
            total_black.to_string().bold().red(),
            "posts with blacklisted tags were not downloaded."
                .bold()
                .red()
        )
    }

    Ok(())
}
