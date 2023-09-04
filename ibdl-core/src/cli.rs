// 20002709
use ibdl_common::{
    post::{extension::Extension, rating::Rating, NameType},
    ImageBoards,
};
use std::path::PathBuf;

use clap::Parser;

use crate::{generate_output_path, generate_output_path_precise, ImageBoardArg, RatingArg};

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

    /// Do not download animated gifs or video files
    #[clap(long, value_parser, default_value_t = false, help_heading = "SAVE")]
    pub no_animated: bool,

    /// Set a max number of posts to download.
    ///
    /// [max: 65535]
    #[clap(short, long, value_parser, help_heading = "DOWNLOAD")]
    pub limit: Option<u16>,

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
    #[clap(long, value_parser, default_value_t = false, help_heading = "SAVE")]
    pub annotate: bool,

    /// Exclude posts with these tags
    #[clap(short, long, value_parser, help_heading = "GENERAL")]
    pub exclude: Vec<String>,

    /// Force the extractor to only fetch posts with the selected extension
    #[clap(long, value_parser, help_heading = "DOWNLOAD")]
    pub force_extension: Option<String>,

    /// Pool ID to download.
    ///
    /// Will always ignore `--id` and cli tags
    #[clap(
        long = "pool",
        value_parser,
        value_name = "ID",
        conflicts_with("tags"),
        conflicts_with("save_file_as_id"),
        requires("precise_output")
    )]
    pub pool_id: Option<u32>,

    /// Download pool posts in reverse order
    ///
    /// Useful when using the download limiter
    #[clap(long = "latest", value_parser, requires("pool_id"))]
    pub latest_first: bool,
}

impl Cli {
    pub fn name_type(&self) -> NameType {
        if self.save_file_as_id {
            NameType::ID
        } else {
            NameType::MD5
        }
    }

    #[inline]
    pub fn selected_ratings(&self) -> Vec<Rating> {
        let mut ratings: Vec<Rating> = Vec::with_capacity(4);
        if self.rating.is_empty() {
            if self.safe_mode {
                ratings.push(Rating::Safe);
            } else {
                ratings.push(Rating::Safe);
                ratings.push(Rating::Questionable);
                ratings.push(Rating::Explicit)
            }
        } else {
            self.rating.iter().for_each(|item| ratings.push(item.0));
        };

        if !self.ignore_unknown {
            ratings.push(Rating::Unknown);
        }
        ratings
    }

    pub fn get_extension(&self) -> Option<Extension> {
        if let Some(ext) = &self.force_extension {
            return Some(Extension::guess_format(ext));
        }
        None
    }

    pub fn generate_save_path(&self) -> Result<PathBuf, std::io::Error> {
        let raw_save_path = if let Some(path) = &self.output {
            path.to_owned()
        } else if let Some(precise_path) = &self.precise_output {
            precise_path.to_owned()
        } else {
            std::env::current_dir()?
        };

        let dirname = if self.output.is_some() {
            assert_eq!(self.precise_output, None);
            generate_output_path(&raw_save_path, *self.imageboard, &self.tags, self.cbz)
        } else if self.precise_output.is_some() {
            assert_eq!(self.output, None);
            generate_output_path_precise(&raw_save_path, self.cbz)
        } else if let Some(id) = self.pool_id {
            if self.cbz {
                raw_save_path.join(format!("{}.cbz", id))
            } else {
                raw_save_path.join(id.to_string())
            }
        } else {
            raw_save_path
        };

        Ok(dirname)
    }
}
