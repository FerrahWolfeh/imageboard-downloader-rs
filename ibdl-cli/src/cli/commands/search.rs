use clap::Args;
use ibdl_common::{
    ImageBoards,
    post::{Post, rating::Rating},
    reqwest::Client,
    tokio::sync::mpsc::{Sender, UnboundedSender},
};
use ibdl_extractors::imageboards::{
    danbooru::DanbooruExtractor, e621::E621Extractor, gelbooru::GelbooruExtractor,
    moebooru::MoebooruExtractor,
};
use ibdl_extractors::prelude::*;

use crate::{
    RatingArg,
    cli::{Cli, extra::auth_imgboard},
    error::CliError,
};

#[derive(Debug, Args)]
pub struct TagSearch {
    /// Tags to search
    #[clap(value_parser, required = true)]
    pub tags: Vec<String>,

    /// Set a max number of posts to download.
    ///
    /// [max: 1000]
    #[clap(short, long, value_parser(clap::value_parser!(u16).range(1..=1000)), help_heading = "DOWNLOAD")]
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

impl TagSearch {
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
        let ratings = self.selected_ratings();

        match args.imageboard.server {
            ImageBoards::Danbooru => {
                let mut unit = DanbooruExtractor::new_with_config(
                    &self.tags,
                    &ratings,
                    self.disable_blacklist,
                    !self.no_animated,
                    args.imageboard.clone(),
                );
                auth_imgboard(args.auth, &mut unit).await?;

                unit.exclude_tags(&self.exclude);

                if let Some(ext) = args.get_extension() {
                    unit.force_extension(ext);
                }

                let client = unit.client();

                let ext_thd = unit.setup_fetch_thread(
                    // Pass the sender for the total count channel
                    channel_tx,
                    self.start_page,
                    self.limit,
                    Some(length_tx),
                );

                Ok((ext_thd, client))
            }
            ImageBoards::E621 => {
                let mut unit = E621Extractor::new_with_config(
                    &self.tags,
                    &ratings,
                    self.disable_blacklist,
                    !self.no_animated,
                    args.imageboard.clone(),
                );
                auth_imgboard(args.auth, &mut unit).await?;

                unit.exclude_tags(&self.exclude);

                if let Some(ext) = args.get_extension() {
                    unit.force_extension(ext);
                }

                let client = unit.client();

                let ext_thd = unit.setup_fetch_thread(
                    // Pass the sender for the total count channel
                    channel_tx,
                    self.start_page,
                    self.limit,
                    Some(length_tx),
                );

                Ok((ext_thd, client))
            }
            ImageBoards::GelbooruV0_2 | ImageBoards::Gelbooru => {
                let mut unit = GelbooruExtractor::new_with_config(
                    &self.tags,
                    &ratings,
                    self.disable_blacklist,
                    !self.no_animated,
                    args.imageboard.clone(),
                );

                unit.exclude_tags(&self.exclude);

                if let Some(ext) = args.get_extension() {
                    unit.force_extension(ext);
                }

                let client = unit.client();

                let ext_thd = unit.setup_fetch_thread(
                    // Pass the sender for the total count channel
                    channel_tx,
                    self.start_page,
                    self.limit,
                    Some(length_tx),
                );

                Ok((ext_thd, client))
            }
            ImageBoards::Moebooru => {
                let mut unit = MoebooruExtractor::new_with_config(
                    &self.tags,
                    &ratings,
                    self.disable_blacklist,
                    !self.no_animated,
                    args.imageboard.clone(),
                );
                let client = unit.client();

                unit.exclude_tags(&self.exclude);

                if let Some(ext) = args.get_extension() {
                    unit.force_extension(ext);
                }

                let ext_thd = unit.setup_fetch_thread(
                    channel_tx,
                    self.start_page,
                    self.limit,
                    Some(length_tx),
                );

                Ok((ext_thd, client))
            }
        }
    }
}
