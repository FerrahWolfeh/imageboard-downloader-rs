use std::path::PathBuf;

use crate::ImageBoards;

pub struct Config {
    imageboard: ImageBoards,
    output_dir: Option<PathBuf>,
    sim_downloads: usize,
    limit: usize,
    save_as_id: bool,
    cbz: bool,
}
