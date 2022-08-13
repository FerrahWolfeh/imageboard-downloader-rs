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
