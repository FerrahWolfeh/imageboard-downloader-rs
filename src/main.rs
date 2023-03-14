use crate::async_path::async_path;
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

    async_path(&args).await
}
