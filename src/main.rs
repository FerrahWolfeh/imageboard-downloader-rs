use crate::async_path::async_path;
use crate::default_path::default_path;
use color_eyre::eyre::Result;
use ibdl_common::tokio;
use ibdl_core::clap::Parser;
use ibdl_core::cli::Cli;

mod async_path;
mod default_path;
mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Cli = Cli::parse();
    env_logger::builder().format_timestamp(None).init();
    color_eyre::install()?;

    if args.async_download {
        async_path(&args).await
    } else {
        default_path(args).await
    }
}
