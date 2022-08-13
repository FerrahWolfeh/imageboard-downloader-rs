use crate::ImageBoards;
use anyhow::Error;
use log::debug;
use std::path::PathBuf;

pub fn generate_out_dir(
    out_dir: Option<PathBuf>,
    tag_string: &String,
    imageboard: ImageBoards,
) -> Result<PathBuf, Error> {
    // If out_dir is not set via cli flags, use current dir
    let place = match out_dir {
        None => std::env::current_dir()?,
        Some(dir) => dir,
    };

    let out = place.join(PathBuf::from(format!(
        "{}/{}",
        imageboard.to_string(),
        tag_string
    )));
    debug!("Target dir: {}", out.display());
    Ok(out)
}

pub fn set_user_agent(imageboard: ImageBoards) -> String {
    let app_name: &str = "Rust Imageboard Downloader";
    let variant = match imageboard {
        ImageBoards::Danbooru => " (by danbooru user FerrahWolfeh)",
        ImageBoards::E621 => " (by e621 user FerrahWolfeh)",
        _ => ""
    };
    let ua = format!("{}/{}{}", app_name, env!("CARGO_PKG_VERSION"), variant);
    debug!("Connecting with user-agent: {}", ua);
    ua

}
