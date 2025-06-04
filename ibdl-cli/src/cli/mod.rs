// 20002709
use ibdl_common::post::{NameType, extension::Extension};
use ibdl_extractors::extractor_config::ServerConfig;
use once_cell::sync::OnceCell;
use std::{collections::HashMap, path::PathBuf};

#[cfg(feature = "cbz")]
use std::path::Path;

use clap::{Parser, Subcommand};

use self::{
    commands::{pool::Pool, post::Post, search::TagSearch},
    extra::validate_imageboard,
};

pub mod commands;
pub(crate) mod extra;

pub static AVAILABLE_SERVERS: OnceCell<HashMap<String, ServerConfig>> = OnceCell::new();

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Search and download posts with tags
    Search(TagSearch),
    /// Download entire pools of posts
    Pool(Pool),
    /// Download a single or multiple specific posts
    Post(Post),
}

#[derive(Parser, Debug)]
#[clap(name = "Imageboard Downloader", author, version, about, long_about = None)]
pub struct Cli {
    #[clap(subcommand)]
    pub mode: Commands,

    /// Specify which website to download from
    ///
    /// Default websites include: ["danbooru", "e621", "gelbooru", "rule34", "realbooru", "konachan"]
    #[clap(short, long, ignore_case = true, default_value_t = ServerConfig::default(), global = true, value_parser = validate_imageboard)]
    pub imageboard: ServerConfig,

    /// Print all available servers and exit
    #[clap(long, global = true)]
    pub servers: bool,

    /// Where to save files (If the path doesn't exist, it will be created.)
    #[clap(short = 'o', value_name = "PATH", help_heading = "SAVE", global = true)]
    pub output: Option<PathBuf>,

    /// Number of simultaneous downloads
    ///
    /// [max: 20]
    #[clap(
        short = 'd',
        value_name = "NUMBER",
        global = true,
        value_parser(clap::value_parser!(u8).range(1..=20)),
        default_value_t = 5,
        help_heading = "DOWNLOAD",
        global = true,
    )]
    pub simultaneous_downloads: u8,

    /// Authenticate to the imageboard website.
    ///
    /// This flag only needs to be set a single time.
    ///
    /// Once authenticated, it's possible to use your blacklist to exclude posts with unwanted tags
    #[clap(short, long, action, help_heading = "GENERAL", global = true)]
    pub auth: bool,

    /// Save files with their ID as filename instead of it's MD5
    ///
    /// If the output dir has the same file downloaded with the MD5 name, it will be renamed to the post's ID
    #[clap(
        long = "id",
        value_parser,
        global = true,
        default_value_t = false,
        help_heading = "SAVE"
    )]
    pub save_file_as_id: bool,

    /// Save posts inside a cbz file.
    ///
    /// Will ask to overwrite the destination file.
    #[cfg(feature = "cbz")]
    #[clap(
        long,
        value_parser,
        default_value_t = false,
        help_heading = "SAVE",
        global = true
    )]
    pub cbz: bool,

    /// Write tags in a txt file next to the downloaded image (for Stable Diffusion training)
    #[clap(
        long,
        value_parser,
        default_value_t = false,
        help_heading = "SAVE",
        global = true
    )]
    pub annotate: bool,

    /// Always overwrite output
    #[clap(
        short = 'y',
        value_parser,
        default_value_t = false,
        help_heading = "SAVE",
        global = true
    )]
    pub overwrite: bool,
}

impl Cli {
    pub const fn name_type(&self) -> NameType {
        if self.save_file_as_id {
            NameType::ID
        } else {
            NameType::MD5
        }
    }

    pub fn get_extension(&self) -> Option<Extension> {
        match &self.mode {
            Commands::Search(args) => {
                if let Some(ext) = &args.force_extension {
                    return Some(Extension::guess_format(ext));
                }
            }
            Commands::Pool(args) => {
                if let Some(ext) = &args.force_extension {
                    return Some(Extension::guess_format(ext));
                }
            }
            Commands::Post(_) => {}
        }
        None
    }

    pub fn generate_save_path(&self) -> Result<PathBuf, std::io::Error> {
        #[cfg(feature = "cbz")]
        if self.cbz {
            // CBZ mode is selected by the user and the feature is enabled.
            // Determine the base path for the CBZ file name.
            let base_name_path = if let Some(output_path) = &self.output {
                output_path.clone()
            } else {
                // No -o provided, use a default name in the current directory.
                std::env::current_dir()?.join("imageboard_download")
            };
            // generate_output_path_precise is only compiled if 'cbz' feature is on.
            // It will append ".cbz" to the base_name_path.
            return Ok(generate_output_path_precise(&base_name_path, true));
        }

        // This block is reached if:
        // 1. The 'cbz' feature is disabled (the #[cfg(feature = "cbz")] block above is removed).
        // 2. The 'cbz' feature is enabled, but self.cbz is false (user didn't pass --cbz).
        // In both cases, we are in folder download mode.
        if let Some(output_path) = &self.output {
            Ok(output_path.clone())
        } else {
            std::env::current_dir()
        }
    }
}

#[cfg(feature = "cbz")]
/// This function creates the destination directory without creating additional ones related to
/// the selected imageboard or tags used.
#[inline]
pub fn generate_output_path_precise(main_path: &Path, cbz_mode: bool) -> PathBuf {
    if cbz_mode {
        return PathBuf::from(&format!("{}.cbz", main_path.display()));
    }
    main_path.to_path_buf()
}
