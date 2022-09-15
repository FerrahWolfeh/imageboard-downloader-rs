use anyhow::Error;
use ibdl_common::colored::Colorize;
use ibdl_common::log::debug;
use ibdl_common::post::rating::Rating;
use ibdl_common::tokio;
use ibdl_common::{clap::Parser, cli::Cli, post::NameType, ImageBoards};
use ibdl_core::queue::{summary::SummaryFile, Queue};
use ibdl_extractors::websites::{
    danbooru::DanbooruExtractor, e621::E621Extractor, gelbooru::GelbooruExtractor,
    moebooru::MoebooruExtractor, Auth, Extractor, MultiWebsite,
};
use spinoff::{Color, Spinner, Spinners};
use std::path::Path;
use tokio::fs::remove_file;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args: Cli = Cli::parse();
    env_logger::builder().format_timestamp(None).init();

    let spinner = Spinner::new_with_stream(
        Spinners::SimpleDotsScrolling,
        "Scanning for posts, please wait".bold().to_string(),
        Color::Blue,
        spinoff::Streams::Stderr,
    );

    let ratings = if args.rating.is_empty() {
        if args.safe_mode {
            vec![Rating::Safe, Rating::Unknown]
        } else {
            vec![
                Rating::Safe,
                Rating::Questionable,
                Rating::Explicit,
                Rating::Unknown,
            ]
        }
    } else {
        args.rating
    };

    debug!("Selected ratings: {:?}", ratings);

    let (mut post_queue, total_black, client) = match args.imageboard {
        ImageBoards::Danbooru => {
            let mut unit = DanbooruExtractor::new(&args.tags, ratings, args.disable_blacklist);
            unit.auth(args.auth).await?;
            let posts = unit.full_search(args.start_page, args.limit).await?;

            debug!("Collected {} valid posts", posts.posts.len());

            (posts, unit.total_removed(), unit.client())
        }
        ImageBoards::E621 => {
            let mut unit = E621Extractor::new(&args.tags, ratings, args.disable_blacklist);
            unit.auth(args.auth).await?;
            let posts = unit.full_search(args.start_page, args.limit).await?;

            debug!("Collected {} valid posts", posts.posts.len());

            (posts, unit.total_removed(), unit.client())
        }
        ImageBoards::Rule34 | ImageBoards::Realbooru | ImageBoards::Gelbooru => {
            let mut unit = GelbooruExtractor::new(&args.tags, ratings, args.disable_blacklist)
                .set_imageboard(args.imageboard)?;
            let posts = unit.full_search(args.start_page, args.limit).await?;

            debug!("Collected {} valid posts", posts.posts.len());

            (posts, unit.total_removed(), unit.client())
        }
        ImageBoards::Konachan => {
            let mut unit = MoebooruExtractor::new(&args.tags, ratings, args.disable_blacklist);
            let posts = unit.full_search(args.start_page, args.limit).await?;

            debug!("Collected {} valid posts", posts.posts.len());

            (posts, unit.total_removed(), unit.client())
        }
    };

    post_queue.posts.shrink_to_fit();

    spinner.clear();

    let place = match &args.output {
        None => std::env::current_dir()?,
        Some(dir) => dir.to_path_buf(),
    };

    let tgs = place.join(Path::new(&format!(
        "{}/{}/{}",
        args.imageboard.to_string(),
        &args.tags.join(" "),
        ".00_download_summary.bin"
    )));

    if args.update && tgs.exists() {
        let summary_file = SummaryFile::read_summary(&tgs).await;
        if let Ok(post) = summary_file {
            debug!("Latest post found: {}", post.last_downloaded);
            post_queue.posts.retain(|c| c.id > post.last_downloaded);
        } else {
            debug!("Summary file is corrupted, ignoring...");
            remove_file(&tgs).await?;
        }
    }

    if post_queue.posts.is_empty() {
        println!("{}", "No posts left to download!".bold());
        return Ok(());
    }

    let post_list = post_queue.posts.clone();

    let mut qw = Queue::new(
        args.imageboard,
        post_queue,
        args.simultaneous_downloads,
        Some(client),
        args.limit,
        args.cbz,
    );

    let nt = match args.save_file_as_id {
        true => NameType::ID,
        false => NameType::MD5,
    };

    let total_down = qw.download(place, nt).await?;

    if !args.cbz {
        let summary = SummaryFile::new(args.imageboard, args.tags, post_list);
        summary.write_summary(&tgs).await?;
    }

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

    Ok(())
}
