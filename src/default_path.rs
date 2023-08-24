use std::path::Path;

use color_eyre::{eyre::Result, owo_colors::OwoColorize};
use ibdl_common::{
    log::debug, post::PostQueue, reqwest::Client, tokio::fs::remove_file, ImageBoards,
};
use ibdl_core::{
    cli::Cli,
    queue::{
        summary::{SummaryFile, SummaryType},
        Queue,
    },
};
use ibdl_extractors::websites::{
    danbooru::DanbooruExtractor, e621::E621Extractor, gelbooru::GelbooruExtractor,
    moebooru::MoebooruExtractor, Extractor, MultiWebsite,
};
use spinoff::{spinners, Color, Spinner};

use crate::utils::{auth_imgboard, print_results};

pub async fn default_path(args: Cli) -> Result<()> {
    let mut spinner = Spinner::new_with_stream(
        spinners::SimpleDotsScrolling,
        "Scanning for posts, please wait".bold().to_string(),
        Color::Blue,
        spinoff::Streams::Stderr,
    );

    let mut nt = args.name_type();

    let (mut post_queue, total_black, client) = search_args(&args).await?;

    if post_queue.posts.is_empty() {
        println!("{}", "No posts left to download!".bold());
        spinner.clear();
        return Ok(());
    }

    post_queue.prepare(args.limit);

    spinner.clear();

    let dirname = args.generate_save_path()?;

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
        args.annotate,
    );

    let total_down = qw.download(dirname, nt).await?;

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

async fn search_args(args: &Cli) -> Result<(PostQueue, u64, Client)> {
    let ratings = args.selected_ratings();

    match *args.imageboard {
        ImageBoards::Danbooru => {
            let mut unit = DanbooruExtractor::new(
                &args.tags,
                &ratings,
                args.disable_blacklist,
                !args.no_animated,
            );
            auth_imgboard(args.auth, &mut unit).await?;

            unit.exclude_tags(&args.exclude);

            if let Some(ext) = args.get_extension() {
                unit.force_extension(ext);
            }

            let posts = unit.full_search(args.start_page, args.limit).await?;

            debug!("Collected {} valid posts", posts.posts.len());

            Ok((posts, unit.total_removed(), unit.client()))
        }
        ImageBoards::E621 => {
            let mut unit = E621Extractor::new(
                &args.tags,
                &ratings,
                args.disable_blacklist,
                !args.no_animated,
            );
            auth_imgboard(args.auth, &mut unit).await?;

            unit.exclude_tags(&args.exclude);

            if let Some(ext) = args.get_extension() {
                unit.force_extension(ext);
            }

            let posts = unit.full_search(args.start_page, args.limit).await?;

            debug!("Collected {} valid posts", posts.posts.len());

            Ok((posts, unit.total_removed(), unit.client()))
        }
        ImageBoards::Rule34 | ImageBoards::Realbooru | ImageBoards::Gelbooru => {
            let mut unit = GelbooruExtractor::new(
                &args.tags,
                &ratings,
                args.disable_blacklist,
                !args.no_animated,
            );

            unit.exclude_tags(&args.exclude)
                .set_imageboard(*args.imageboard);

            let posts = unit.full_search(args.start_page, args.limit).await?;

            if let Some(ext) = args.get_extension() {
                unit.force_extension(ext);
            }

            debug!("Collected {} valid posts", posts.posts.len());

            Ok((posts, unit.total_removed(), unit.client()))
        }
        ImageBoards::Konachan => {
            let mut unit = MoebooruExtractor::new(
                &args.tags,
                &ratings,
                args.disable_blacklist,
                !args.no_animated,
            );

            unit.exclude_tags(&args.exclude);

            let posts = unit.full_search(args.start_page, args.limit).await?;

            if let Some(ext) = args.get_extension() {
                unit.force_extension(ext);
            }

            debug!("Collected {} valid posts", posts.posts.len());

            Ok((posts, unit.total_removed(), unit.client()))
        }
    }
}
