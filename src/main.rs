use anyhow::Error;
use ibdl_common::colored::Colorize;
use ibdl_common::log::debug;
use ibdl_common::post::rating::Rating;
use ibdl_common::post::{NameType, PostQueue};
use ibdl_common::reqwest::Client;
use ibdl_common::tokio;
use ibdl_common::ImageBoards;
use ibdl_core::clap::Parser;
use ibdl_core::cli::Cli;
use ibdl_core::queue::summary::SummaryType;
use ibdl_core::queue::{summary::SummaryFile, Queue};
use ibdl_core::{generate_output_path, generate_output_path_precise};
use ibdl_extractors::websites::{
    danbooru::DanbooruExtractor, e621::E621Extractor, gelbooru::GelbooruExtractor,
    moebooru::MoebooruExtractor, Auth, Extractor, MultiWebsite,
};
use spinoff::{spinners, Color, Spinner};
use std::path::Path;
use tokio::fs::remove_file;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args: Cli = Cli::parse();
    env_logger::builder().format_timestamp(None).init();

    let spinner = Spinner::new_with_stream(
        spinners::SimpleDotsScrolling,
        "Scanning for posts, please wait".bold().to_string(),
        Color::Blue,
        spinoff::Streams::Stderr,
    );

    let mut nt = if args.save_file_as_id {
        NameType::ID
    } else {
        NameType::MD5
    };

    let (mut post_queue, total_black, client) = search_args(&args).await?;

    if post_queue.posts.is_empty() {
        println!("{}", "No posts left to download!".bold());
        spinner.clear();
        return Ok(());
    }

    post_queue.prepare(args.limit);

    spinner.clear();

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
        raw_save_path.to_path_buf()
    };

    let summary_path = dirname.join(Path::new(".00_download_summary.bin"));

    if args.update && summary_path.exists() {
        let summary_file = SummaryFile::read_summary(&summary_path, SummaryType::ZSTDBincode).await;
        if let Ok(post) = summary_file {
            debug!("Latest post found: {}", post.last_downloaded);
            post_queue.posts.retain(|c| c.id > post.last_downloaded);
            post_queue.posts.shrink_to_fit();
            nt = post.name_mode;
        } else {
            debug!("Summary file is corrupted, ignoring...");
            remove_file(&summary_path).await?;
        }
    }

    let post_list = post_queue.posts.clone();

    let qw = Queue::new(
        *args.imageboard,
        post_queue,
        args.simultaneous_downloads,
        Some(client),
        args.cbz,
    );

    let total_down = qw.download(dirname, nt, args.annotate).await?;

    if !args.cbz {
        let summary = SummaryFile::new(
            *args.imageboard,
            &args.tags,
            &post_list,
            nt,
            SummaryType::ZSTDBincode,
        );
        summary.write_summary(&summary_path).await?;
    }

    print_results(total_down, total_black);

    Ok(())
}

fn print_results(total_down: u64, total_black: u64) {
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

async fn search_args(args: &Cli) -> Result<(PostQueue, u64, Client), Error> {
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

    match *args.imageboard {
        ImageBoards::Danbooru => {
            let mut unit = DanbooruExtractor::new(&args.tags, &ratings, args.disable_blacklist);
            unit.auth(args.auth).await?;
            let posts = unit.full_search(args.start_page, args.limit).await?;

            debug!("Collected {} valid posts", posts.posts.len());

            Ok((posts, unit.total_removed(), unit.client()))
        }
        ImageBoards::E621 => {
            let mut unit = E621Extractor::new(&args.tags, &ratings, args.disable_blacklist);
            unit.auth(args.auth).await?;
            let posts = unit.full_search(args.start_page, args.limit).await?;

            debug!("Collected {} valid posts", posts.posts.len());

            Ok((posts, unit.total_removed(), unit.client()))
        }
        ImageBoards::Rule34 | ImageBoards::Realbooru | ImageBoards::Gelbooru => {
            let mut unit = GelbooruExtractor::new(&args.tags, &ratings, args.disable_blacklist)
                .set_imageboard(*args.imageboard)?;
            let posts = unit.full_search(args.start_page, args.limit).await?;

            debug!("Collected {} valid posts", posts.posts.len());

            Ok((posts, unit.total_removed(), unit.client()))
        }
        ImageBoards::Konachan => {
            let mut unit = MoebooruExtractor::new(&args.tags, &ratings, args.disable_blacklist);
            let posts = unit.full_search(args.start_page, args.limit).await?;

            debug!("Collected {} valid posts", posts.posts.len());

            Ok((posts, unit.total_removed(), unit.client()))
        }
    }
}
