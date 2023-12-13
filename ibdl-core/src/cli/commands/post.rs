use std::path::PathBuf;

use clap::Args;
use ibdl_common::{
    log::warn,
    post::Post as Pst,
    reqwest::Client,
    tokio::{
        fs,
        sync::mpsc::{Sender, UnboundedSender},
    },
    ImageBoards,
};
use ibdl_extractors::websites::{danbooru::DanbooruExtractor, e621::E621Extractor};
use ibdl_extractors::{prelude::*, websites::PostFetchMethod};
use owo_colors::OwoColorize;

use crate::{
    cli::{extra::auth_imgboard, Cli},
    error::CliError,
};

#[derive(Debug, Args)]
pub struct Post {
    /// Download a specific post/image
    #[clap(
        value_parser,
        value_name = "POST IDs",
        conflicts_with("post_file"),
        required = true
    )]
    posts: Vec<u32>,

    /// Download a list of posts from a file (one post id per line)
    #[clap(
        long = "post_file",
        value_name = "FILE PATH",
        value_parser,
        conflicts_with("posts")
    )]
    post_file: Option<PathBuf>,
}

impl Post {
    pub async fn init_extractor(
        &self,
        args: &Cli,
        channel_tx: UnboundedSender<Pst>,
        length_tx: Sender<u64>,
    ) -> Result<(ExtractorThreadHandle, Client), CliError> {
        match *args.imageboard {
            ImageBoards::Danbooru => {
                let mut unit = DanbooruExtractor::new(&[""], &[], true, true);
                auth_imgboard(args.auth, &mut unit).await?;

                let client = unit.client();

                let ext_thd = {
                    if !self.posts.is_empty() {
                        unit.setup_async_post_fetch(
                            channel_tx,
                            PostFetchMethod::Multiple(self.posts.clone()),
                            length_tx,
                        )
                    } else if let Some(path) = &self.post_file {
                        let posts = fs::read_to_string(&path).await?;
                        let ids = Vec::from_iter(posts.lines().filter_map(|line| {
                            if let Ok(id) = line.parse::<u32>() {
                                Some(id)
                            } else {
                                warn!(
                                    "Failed to parse line {} into a post id",
                                    line.bright_blue().bold()
                                );
                                None
                            }
                        }));

                        if ids.is_empty() {
                            return Err(CliError::NoPostsInInput);
                        }

                        unit.setup_async_post_fetch(
                            channel_tx,
                            PostFetchMethod::Multiple(ids),
                            length_tx,
                        )
                    } else {
                        return Err(CliError::NoPostsInInput);
                    }
                };

                Ok((ext_thd, client))
            }
            ImageBoards::E621 => {
                let mut unit = E621Extractor::new(&[""], &[], true, true);
                auth_imgboard(args.auth, &mut unit).await?;

                let client = unit.client();
                let ext_thd = {
                    if !self.posts.is_empty() {
                        unit.setup_async_post_fetch(
                            channel_tx,
                            PostFetchMethod::Multiple(self.posts.clone()),
                            length_tx,
                        )
                    } else if let Some(path) = &self.post_file {
                        let posts = fs::read_to_string(&path).await?;
                        let ids = Vec::from_iter(posts.lines().filter_map(|line| {
                            if let Ok(id) = line.parse::<u32>() {
                                Some(id)
                            } else {
                                warn!(
                                    "Failed to parse line {} into a post id",
                                    line.bright_blue().bold()
                                );
                                None
                            }
                        }));

                        if ids.is_empty() {
                            return Err(CliError::NoPostsInInput);
                        }

                        unit.setup_async_post_fetch(
                            channel_tx,
                            PostFetchMethod::Multiple(ids),
                            length_tx,
                        )
                    } else {
                        return Err(CliError::NoPostsInInput);
                    }
                };

                Ok((ext_thd, client))
            }
            ImageBoards::Rule34 | ImageBoards::Realbooru | ImageBoards::Gelbooru => {
                Err(CliError::ExtractorUnsupportedMode)
                // let mut unit = GelbooruExtractor::new(
                //     &args.tags,
                //     &ratings,
                //     args.disable_blacklist,
                //     !args.no_animated,
                // );

                // unit.exclude_tags(&args.exclude)
                //     .set_imageboard(*args.imageboard);

                // if let Some(ext) = args.get_extension() {
                //     unit.force_extension(ext);
                // }

                // let client = unit.client();

                // let ext_thd = unit.setup_fetch_thread(
                //     channel_tx,
                //     args.start_page,
                //     args.limit,
                //     Some(length_tx),
                // );

                // Ok((ext_thd, client))
            }
            ImageBoards::Konachan => {
                Err(CliError::ExtractorUnsupportedMode)

                // let mut unit = MoebooruExtractor::new(
                //     &args.tags,
                //     &ratings,
                //     args.disable_blacklist,
                //     !args.no_animated,
                // );
                // let client = unit.client();

                // unit.exclude_tags(&args.exclude);

                // if let Some(ext) = args.get_extension() {
                //     unit.force_extension(ext);
                // }

                // let ext_thd = unit.setup_fetch_thread(
                //     channel_tx,
                //     args.start_page,
                //     args.limit,
                //     Some(length_tx),
                // );

                // Ok((ext_thd, client))
            }
        }
    }
}
