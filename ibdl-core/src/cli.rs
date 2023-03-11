// 20002709
use ibdl_common::ImageBoards;
use std::path::PathBuf;

use clap::Parser;

use crate::{ImageBoardArg, RatingArg};

#[derive(Parser, Debug)]
#[clap(name = "Imageboard Downloader", author, version, about, long_about = None)]
pub struct Cli {
    /// Tags to search
    #[clap(value_parser, required = true)]
    pub tags: Vec<String>,

    /// Specify which website to download from
    #[clap(short, long, value_enum, ignore_case = true, default_value_t = ImageBoardArg(ImageBoards::Danbooru))]
    pub imageboard: ImageBoardArg,

    /// Where to save downloaded files
    #[clap(
        short,
        long,
        value_name = "PATH",
        help_heading = "SAVE",
        conflicts_with("precise_output")
    )]
    pub output: Option<PathBuf>,

    /// Use this option to save files directly to the specified directory without creating additional dirs
    #[clap(
        short = 'O',
        value_name = "PATH",
        help_heading = "SAVE",
        conflicts_with("output")
    )]
    pub precise_output: Option<PathBuf>,

    /// Number of simultaneous downloads
    ///
    /// [max: 20]
    #[clap(
        short = 'd',
        value_name = "NUMBER",
        value_parser(clap::value_parser!(u8).range(1..=20)),
        default_value_t = 5,
        help_heading = "DOWNLOAD"
    )]
    pub simultaneous_downloads: u8,

    /// Authenticate to the imageboard website.
    ///
    /// This flag only needs to be set a single time.
    ///
    /// Once authenticated, it's possible to use your blacklist to exclude posts with unwanted tags
    #[clap(short, long, action, help_heading = "GENERAL")]
    pub auth: bool,

    /// Download images from the safe version of the selected Imageboard.
    ///
    /// Useful if you only want to download posts with "safe" rating.
    #[clap(long, action, default_value_t = false, help_heading = "GENERAL")]
    pub safe_mode: bool,

    /// Save files with their ID as filename instead of it's MD5
    ///
    /// If the output dir has the same file downloaded with the MD5 name, it will be renamed to the post's ID
    #[clap(
        long = "id",
        value_parser,
        default_value_t = false,
        help_heading = "SAVE"
    )]
    pub save_file_as_id: bool,

    /// Set a max number of posts to download.
    ///
    /// [max: 65535]
    #[clap(short, long, value_parser, help_heading = "DOWNLOAD")]
    pub limit: Option<u16>,

    /// Enable experimental async downloader (currently only available for Danbooru)
    #[clap(
        long = "async",
        value_parser,
        default_value_t = false,
        help_heading = "DOWNLOAD"
    )]
    pub async_download: bool,

    /// Disable blacklist filtering
    #[clap(long, value_parser, default_value_t = false, help_heading = "GENERAL")]
    pub disable_blacklist: bool,

    /// Save posts inside a cbz file.
    ///
    /// Will always overwrite the destination file.
    #[clap(long, value_parser, default_value_t = false, help_heading = "SAVE")]
    pub cbz: bool,

    /// Select from which page to start scanning posts
    ///
    /// [max: 65535]
    #[clap(
        short,
        long,
        value_parser,
        help_heading = "DOWNLOAD",
        value_name = "PAGE"
    )]
    pub start_page: Option<u16>,

    /// Download only the latest images for tag selection.
    ///
    /// Will not re-download already present or deleted images from destination directory
    #[clap(
        short,
        long,
        value_parser,
        default_value_t = false,
        help_heading = "SAVE"
    )]
    pub update: bool,

    /// Download posts with the selected rating. Can be used multiple times to download posts with other ratings
    #[clap(
        short,
        long,
        value_parser,
        help_heading = "GENERAL",
        conflicts_with("safe_mode")
    )]
    pub rating: Vec<RatingArg>,

    /// Do not download posts with an unknown rating
    #[clap(long, value_parser, default_value_t = false, help_heading = "SAVE")]
    pub ignore_unknown: bool,

    /// Write tags in a txt file next to the downloaded image (for Stable Diffusion training)
    #[clap(
        long,
        value_parser,
        default_value_t = false,
        help_heading = "SAVE",
        conflicts_with("cbz")
    )]
    pub annotate: bool,
}
