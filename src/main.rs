#![deny(clippy::all)]
use color_eyre::eyre::{bail, Result};
use color_eyre::owo_colors::OwoColorize;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use ibdl_common::tokio::sync::mpsc::{channel, unbounded_channel};
use ibdl_common::tokio::{self, join};
use ibdl_core::async_queue::Queue;
use ibdl_core::clap::Parser;
use ibdl_core::cli::{Cli, Commands, AVAILABLE_SERVERS};
use ibdl_extractors::prelude::ExtractorFeatures;
use once_cell::sync::Lazy;
use std::process::exit;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

static POST_COUNTER: Lazy<Arc<AtomicU64>> = Lazy::new(|| Arc::new(AtomicU64::new(0)));

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
        let conf_exists = Confirm::with_theme(&ColorfulTheme::default())
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

    let (channel_tx, channel_rx) = unbounded_channel();

    let (length_sender, length_channel) = channel(args.simultaneous_downloads as usize);
    let mut is_pool = false;

    let (ext, client) = match &args.mode {
        Commands::Search(com) => com.init_extractor(&args, channel_tx, length_sender).await?,
        Commands::Pool(com) => {
            is_pool = true;
            com.init_extractor(&args, channel_tx, length_sender).await?
        }
        Commands::Post(com) => com.init_extractor(&args, channel_tx, length_sender).await?,
    };

    let qw = Queue::new(
        args.imageboard.clone(),
        args.simultaneous_downloads,
        Some(client),
        args.cbz,
        is_pool,
        args.name_type(),
        args.annotate,
    );

    let asd = qw.setup_async_downloader(dirname, POST_COUNTER.clone(), channel_rx, length_channel);

    let (Ok(removed), Ok(results)) = join!(ext, asd) else {
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
