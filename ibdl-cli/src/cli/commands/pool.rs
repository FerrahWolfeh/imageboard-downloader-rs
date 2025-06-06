use clap::Args;
use ibdl_common::{
    ImageBoards,
    post::{Post, rating::Rating},
};
use ibdl_extractors::extractor::PostExtractor;
use ibdl_extractors::prelude::*;

#[cfg(feature = "danbooru")]
use ibdl_extractors::imageboards::prelude::DanbooruApi;
#[cfg(feature = "e621")]
use ibdl_extractors::imageboards::prelude::E621Api;
use reqwest::Client;
use tokio::sync::mpsc::{Sender, UnboundedSender};

use crate::cli::extra::init_blacklist;
// Enable the auth import only when imageboards that support both pools and auth are enabled.
// Currently, Danbooru and E621 fit this.
#[cfg(any(feature = "danbooru", feature = "e621"))]
// #[allow(unused_imports)] // May be unused if only one of danbooru/e621 is enabled
use crate::{
    RatingArg,
    cli::{Cli, extra::auth_imgboard},
    error::CliError,
};

#[derive(Debug, Args)]
pub struct Pool {
    /// Pool ID to download.
    ///
    /// Will always ignore `--id` and cli tags
    #[clap(
        value_parser,
        value_name = "ID",
        conflicts_with("save_file_as_id"),
        requires("output")
    )]
    pub pool_id: u32,

    /// Download pool posts in reverse order
    ///
    /// Useful when using the download limiter
    #[clap(long = "latest", value_parser, requires("pool_id"))]
    pub latest_first: bool,

    /// Set a max number of posts to download.
    ///
    /// [max: 65535]
    #[clap(short, long, value_parser, help_heading = "DOWNLOAD")]
    pub limit: Option<u16>,

    /// Disable blacklist filtering
    #[clap(long, value_parser, default_value_t = false, help_heading = "GENERAL")]
    pub disable_blacklist: bool,

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

    /// Exclude posts with these tags
    #[clap(short, long, value_parser, help_heading = "GENERAL")]
    pub exclude: Vec<String>,

    /// Force the extractor to only fetch posts with the selected extension
    #[clap(long, value_parser, help_heading = "DOWNLOAD", global = true)]
    pub force_extension: Option<String>,

    /// Do not download animated gifs or video files
    #[clap(
        long,
        value_parser,
        default_value_t = false,
        help_heading = "SAVE",
        global = true
    )]
    pub no_animated: bool,

    /// Download images from the safe version of the selected Imageboard.
    ///
    /// Useful if you only want to download posts with "safe" rating.
    #[clap(
        long,
        action,
        default_value_t = false,
        help_heading = "GENERAL",
        global = true
    )]
    pub safe_mode: bool,

    /// Download posts with the selected rating. Can be used multiple times to download posts with other ratings
    #[clap(
        short,
        long,
        value_parser,
        help_heading = "GENERAL",
        conflicts_with("safe_mode"),
        global = true
    )]
    pub rating: Vec<RatingArg>,

    /// Do not download posts with an unknown rating
    #[clap(
        long,
        value_parser,
        default_value_t = false,
        help_heading = "SAVE",
        global = true
    )]
    pub ignore_unknown: bool,
}

impl Pool {
    #[inline]
    fn selected_ratings(&self) -> Vec<Rating> {
        let mut ratings: Vec<Rating> = Vec::with_capacity(4);
        if self.rating.is_empty() {
            ratings.push(Rating::Safe);

            if !self.safe_mode {
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

    pub async fn init_extractor(
        &self,
        args: &Cli,
        channel_tx: UnboundedSender<Post>,
        length_tx: Sender<u64>,
    ) -> Result<(ExtractorThreadHandle, Client), CliError> {
        let global_blacklist = init_blacklist().await?;

        let ratings = self.selected_ratings();

        match args.imageboard.server {
            #[cfg(feature = "danbooru")]
            ImageBoards::Danbooru => {
                let mut unit = PostExtractor::new(
                    &Vec::<String>::new(), // Tags are not used for pool downloads directly by PostExtractor::new
                    &global_blacklist,
                    &ratings,
                    self.disable_blacklist,
                    !self.no_animated,
                    DanbooruApi::new(),      // site_api
                    args.imageboard.clone(), // server_cfg_param
                );

                auth_imgboard(args.auth, &mut unit).await?;

                unit.exclude_tags(&self.exclude);

                if let Some(ext) = args.get_extension() {
                    unit.force_extension(ext);
                }

                unit.setup_pool_download(Some(self.pool_id), self.latest_first);

                let client = unit.client();

                let ext_thd = unit.setup_fetch_thread(
                    channel_tx,
                    self.start_page,
                    self.limit,
                    Some(length_tx),
                );

                Ok((ext_thd, client))
            }
            #[cfg(feature = "e621")]
            ImageBoards::E621 => {
                let mut unit = PostExtractor::new(
                    &Vec::<String>::new(),
                    &global_blacklist,
                    &ratings,
                    self.disable_blacklist,
                    !self.no_animated,
                    E621Api::new(),
                    args.imageboard.clone(), // server_cfg_param
                );

                auth_imgboard(args.auth, &mut unit).await?;

                unit.exclude_tags(&self.exclude);

                if let Some(ext) = args.get_extension() {
                    unit.force_extension(ext);
                }

                unit.setup_pool_download(Some(self.pool_id), self.latest_first);

                let client = unit.client();

                let ext_thd = unit.setup_fetch_thread(
                    channel_tx,
                    self.start_page,
                    self.limit,
                    Some(length_tx),
                );

                Ok((ext_thd, client))
            }
            #[cfg(feature = "gelbooru")]
            ImageBoards::GelbooruV0_2 | ImageBoards::Gelbooru => {
                // GelbooruApi::features() does not include PoolExtract
                Err(CliError::ExtractorUnsupportedMode)
            }
            #[cfg(feature = "moebooru")]
            ImageBoards::Moebooru => {
                // MoebooruApi::features() does not include PoolExtract
                Err(CliError::ExtractorUnsupportedMode)
            }
            #[allow(unreachable_patterns)] // To suppress warnings if all features are enabled
            _ => {
                #[cfg(any(
                    feature = "danbooru",
                    feature = "e621",
                    feature = "gelbooru",
                    feature = "moebooru"
                ))]
                {
                    Err(CliError::ImageboardNotEnabled {
                        imageboard: args.imageboard.server.to_string(),
                    })
                }
                #[cfg(not(any(
                    feature = "danbooru",
                    feature = "e621",
                    feature = "gelbooru",
                    feature = "moebooru"
                )))]
                {
                    Err(CliError::NoImageboardsEnabled)
                }
            }
        }
    }
}
