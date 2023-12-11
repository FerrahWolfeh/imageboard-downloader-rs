use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use color_eyre::eyre::{bail, Result};
use color_eyre::owo_colors::OwoColorize;
use ibdl_common::tokio::sync::mpsc::{channel, unbounded_channel};
use ibdl_common::tokio::{self, join};
use ibdl_core::async_queue::Queue;
use ibdl_core::clap::Parser;
use ibdl_core::cli::{Cli, Commands};
use once_cell::sync::Lazy;

static POST_COUNTER: Lazy<Arc<AtomicU64>> = Lazy::new(|| Arc::new(AtomicU64::new(0)));

#[tokio::main]
async fn main() -> Result<()> {
    let args: Cli = Cli::parse();
    env_logger::builder().format_timestamp(None).init();
    color_eyre::install()?;

    let (channel_tx, channel_rx) = unbounded_channel();

    let (length_sender, length_channel) = channel(8);
    let mut is_pool = false;

    let (ext, client) = match &args.mode {
        Commands::Search(com) => com.init_extractor(&args, channel_tx, length_sender).await?,
        Commands::Pool(com) => {
            is_pool = true;
            com.init_extractor(&args, channel_tx, length_sender).await?
        }
        Commands::Post(com) => com.init_extractor(&args, channel_tx).await?,
    };

    let dirname = args.generate_save_path()?;

    let qw = Queue::new(
        *args.imageboard,
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

pub fn print_results(total_down: u64, total_black: u64) {
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
