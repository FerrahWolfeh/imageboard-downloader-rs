use std::path::PathBuf;

use clap::Parser;

use crate::{post::rating::Rating, ImageBoards};

#[derive(Parser, Debug)]
#[clap(name = "Imageboard Downloader", author, version, about, long_about = None)]
pub struct Cli {
    /// Tags to search
    #[clap(value_parser, required = true)]
    pub tags: Vec<String>,

    /// Specify which website to download from
    #[clap(short, long, arg_enum, ignore_case = true, default_value_t = ImageBoards::Danbooru)]
    pub imageboard: ImageBoards,

    /// Where to save downloaded files
    #[clap(
        short,
        long,
        parse(from_os_str),
        value_name = "PATH",
        help_heading = "SAVE"
    )]
    pub output: Option<PathBuf>,

    /// Number of simultaneous downloads
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

    /// Set a max number of posts to download
    #[clap(short, long, value_parser, help_heading = "DOWNLOAD")]
    pub limit: Option<usize>,

    /// Disable blacklist filtering
    #[clap(long, value_parser, default_value_t = false, help_heading = "GENERAL")]
    pub disable_blacklist: bool,

    /// Save posts inside a cbz file.
    ///
    /// Will always overwrite the destination file.
    #[clap(long, value_parser, default_value_t = false, help_heading = "SAVE")]
    pub cbz: bool,

    /// Select from which page to start scanning posts
    #[clap(
        short,
        long,
        value_parser,
        help_heading = "DOWNLOAD",
        value_name = "PAGE"
    )]
    pub start_page: Option<usize>,

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

    #[clap(
        short,
        long,
        value_parser,
        help_heading = "GENERAL",
        conflicts_with("safe_mode")
    )]
    pub rating: Vec<Rating>,
}
