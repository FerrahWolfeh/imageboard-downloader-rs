#![deny(clippy::all)]
use color_eyre::eyre::{Result, bail};
use color_eyre::owo_colors::OwoColorize;
use dialoguer::Confirm;
use ibdl_cli::cli::{AVAILABLE_SERVERS, Cli, Commands};
use ibdl_cli::progress_bars::IndicatifProgressHandler; // Import the CLI progress handler
use ibdl_core::async_queue::Queue;
use ibdl_core::async_queue::QueueOpts;
use ibdl_core::clap::Parser;
use ibdl_core::progress::ProgressListener;
use ibdl_extractors::prelude::ExtractorFeatures;
use std::process::exit;
use std::sync::Arc;
use tokio::sync::mpsc::{channel, unbounded_channel};
use tokio::{self, join};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Cli = Cli::parse();

    if args.servers {
        print_servers()
    }

    env_logger::builder().format_timestamp(None).init();
    color_eyre::install()?;

    let dirname = args.generate_save_path()?;

    if (dirname.exists() && (dirname.is_file() || dirname.read_dir()?.next().is_some()))
        && !args.overwrite
    {
        let conf_exists = Confirm::new()
            .with_prompt(format!(
                "The path {} is not empty or already exists. Do you want to continue?",
                dirname.display().bold().blue().italic()
            ))
            .wait_for_newline(true)
            .interact()?;

        if !conf_exists {
            println!("{}", "Download cancelled".bold().blue());
            exit(0);
        }
    }

    // Channel for posts from extractor to queue
    let (posts_sender, posts_receiver) = unbounded_channel();
    // Channel for total post count from extractor to progress handler
    let (length_sender, mut length_receiver) = channel::<u64>(1);
    let mut is_pool = false;

    // Create the progress handler instance
    // The initial length will be set by the extractor via the listener
    let progress_handler = Arc::new(IndicatifProgressHandler::new(
        0,
        args.imageboard.clone().server,
    ));

    // Task to update the main progress bar's total length
    let progress_total_updater_task = tokio::spawn({
        let progress_handler = progress_handler.clone();
        async move {
            while let Some(total_posts) = length_receiver.recv().await {
                progress_handler.inc_main_total(total_posts);
            }
            // Receiver will be dropped when sender is dropped by the extractor or extractor finishes
        }
    });

    let (ext, client) = match &args.mode {
        Commands::Search(com) => {
            com.init_extractor(&args, posts_sender, length_sender)
                .await?
        }
        Commands::Pool(com) => {
            is_pool = true;
            com.init_extractor(&args, posts_sender, length_sender)
                .await?
        }
        Commands::Post(com) => {
            com.init_extractor(&args, posts_sender, length_sender)
                .await?
        }
    };

    let output_options = QueueOpts {
        #[cfg(feature = "cbz")]
        save_as_cbz: args.cbz,
        #[cfg(not(feature = "cbz"))]
        save_as_cbz: false, // If CBZ feature is off, this must be false
        pool_download: is_pool,
        name_type: args.name_type(),
        annotate: args.annotate,
    };

    let qw = Queue::new(
        args.imageboard.clone(),
        args.simultaneous_downloads,
        Some(client),
        output_options,
        Some(progress_handler.clone()), // Pass the progress handler
    );

    let downloader_task = qw.setup_async_downloader(dirname, posts_receiver);

    let (Ok(removed), Ok(results), Ok(_)) =
        join!(ext, downloader_task, progress_total_updater_task)
    else {
        bail!("Failed starting threads!")
    };

    print_results(results?, removed?);

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
            "found posts with blacklisted tags were not downloaded."
                .bold()
                .red()
        );
    }
}

fn print_servers() {
    println!(
        "{}\n----------------",
        "Available Servers:".underline().bold().blue()
    );

    for (srv, data) in AVAILABLE_SERVERS.get().unwrap() {
        let mut features = Vec::with_capacity(4);

        let ext_feat = data.extractor_features();

        if ext_feat.contains(ExtractorFeatures::Auth) {
            features.push("Auth");
        }

        if ext_feat.contains(ExtractorFeatures::AsyncFetch) {
            features.push("Async Fetch");
        }

        if ext_feat.contains(ExtractorFeatures::TagSearch) {
            features.push("Tag Search");
        }

        if ext_feat.contains(ExtractorFeatures::SinglePostFetch) {
            features.push("Specific Post Fetch");
        }

        if ext_feat.contains(ExtractorFeatures::PoolDownload) {
            features.push("Pool Download");
        }

        println!(
            "{:<16} - {}:\n - {} {}\n - {} {}\n - {} {}\n - {} {:?}\n",
            format!("[{}]", srv),
            data.pretty_name.bold().green(),
            "API Type:".bold().blue(),
            data.server.to_string().bold().purple().underline(),
            "Base URL:".bold().blue(),
            data.base_url.bold().purple().underline(),
            "Max Post Limit:".bold().blue(),
            data.max_post_limit.bold().yellow(),
            "Available features:".bold().blue(),
            features,
        )
    }

    exit(0)
}
