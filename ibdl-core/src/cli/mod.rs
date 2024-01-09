// 20002709
use ibdl_common::post::{extension::Extension, NameType};
use ibdl_extractors::extractor_config::ServerConfig;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::generate_output_path_precise;

use self::{
    commands::{pool::Pool, post::Post, search::TagSearch},
    extra::validate_imageboard,
};

pub mod commands;
pub(crate) mod extra;

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
    /// Default websites include: ["danbooru", "e621", "gelbooru", "rule34", "realbooru", "konachan"] [Default: "danbooru"]
    #[clap(short, long, ignore_case = true, default_value_t = ServerConfig::default(), global = true, value_parser = validate_imageboard)]
    pub imageboard: ServerConfig,

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
}

impl Cli {
    pub fn name_type(&self) -> NameType {
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
        let raw_save_path = if let Some(precise_path) = &self.output {
            precise_path.to_owned()
        } else {
            std::env::current_dir()?
        };

        let dirname = if self.output.is_some() {
            generate_output_path_precise(&raw_save_path, self.cbz)
        } else {
            raw_save_path
        };

        Ok(dirname)
    }
}
