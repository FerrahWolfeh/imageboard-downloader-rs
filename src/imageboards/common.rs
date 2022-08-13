//! Common functions for all imageboard downloader modules.
use crate::ImageBoards;
use anyhow::Error;
use log::debug;
use std::path::PathBuf;

/// Checks if ```output_dir``` is set in cli args then returns a ```PathBuf``` pointing to where the files will be downloaded.
///
/// In case the user does not set a value with the ```-o``` flag, the function will default to the current dir where the program is running.
///
/// The path chosen will always end with the imageboard name followed by the tags used.
///
/// ```rust
/// let tags = join_tags!(["kroos_(arknights)", "weapon"]);
/// let path = Some(PathBuf::from("./"));
///
/// let out_dir = generate_out_dir(path, &tags, ImageBoards::Danbooru).unwrap();
///
/// assert_eq!(PathBuf::from("./danbooru/kroos_(arknights)+weapon"), out_dir);
/// ```
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
