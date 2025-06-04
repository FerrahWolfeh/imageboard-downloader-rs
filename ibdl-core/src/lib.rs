#![deny(clippy::nursery)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::struct_field_names)]
pub use clap;
use ibdl_common::ImageBoards;
use std::path::{Path, PathBuf};

pub mod async_queue;
pub mod error;
pub mod progress;

#[inline]
pub fn generate_output_path(
    main_path: &Path,
    imageboard: ImageBoards,
    tags: &[String],
    cbz_mode: bool,
    pool_id: Option<u32>,
) -> PathBuf {
    let tag_string = tags.join(" ");
    let tag_path_string = if tag_string.contains("fav:") {
        String::from("Favorites")
    } else if cfg!(windows) {
        tag_string.replace(':', "_")
    } else if let Some(id) = pool_id {
        id.to_string()
    } else {
        tag_string
    };

    let pbuf = main_path.join(Path::new(&imageboard.to_string()));

    if cbz_mode {
        return pbuf.join(Path::new(&format!("{}.cbz", tag_path_string)));
    }
    pbuf.join(Path::new(&tag_path_string))
}
