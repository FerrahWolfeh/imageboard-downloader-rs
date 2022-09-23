use ibdl_common::ImageBoards;
use std::path::{Path, PathBuf};

pub mod progress_bars;
pub mod queue;

#[inline]
pub fn generate_output_path(
    main_path: PathBuf,
    imageboard: ImageBoards,
    tags: &[String],
    cbz_mode: bool,
) -> PathBuf {
    let tag_string = tags.join(" ");
    let tag_path_string = if tag_string.contains("fav:") {
        String::from("Favorites")
    } else if cfg!(windows) {
        tag_string.replace(':', "_")
    } else {
        tag_string
    };

    let pbuf = main_path.join(Path::new(&imageboard.to_string()));

    if cbz_mode {
        return pbuf.join(Path::new(&format!("{}.cbz", tag_path_string)));
    }
    pbuf.join(Path::new(&tag_path_string))
}
