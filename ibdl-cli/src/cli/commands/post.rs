use std::path::PathBuf;

use clap::Args;
use ibdl_common::{
    ImageBoards,
    log::warn,
    post::Post as Pst,
    reqwest::Client,
    tokio::{
        fs,
        sync::mpsc::{Sender, UnboundedSender},
    },
};
use ibdl_extractors::prelude::*;

use ibdl_extractors::extractor::PostExtractor;

#[cfg(feature = "danbooru")]
use ibdl_extractors::imageboards::prelude::DanbooruApi;
#[cfg(feature = "e621")]
use ibdl_extractors::imageboards::prelude::E621Api;
#[cfg(feature = "gelbooru")]
use ibdl_extractors::imageboards::prelude::GelbooruApi;

use owo_colors::OwoColorize;

// Enable the auth import only when imageboards that support it are enabled
#[cfg(any(feature = "danbooru", feature = "e621"))]
use crate::{
    cli::{Cli, extra::auth_imgboard},
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
        match args.imageboard.server {
            ImageBoards::Danbooru => {
                let mut unit = PostExtractor::new(
                    &[""],
                    &[],
                    true,
                    true,
                    DanbooruApi::new(),
                    args.imageboard.clone(),
                );

                // Authenticate if the feature is enabled and auth is requested
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
                            line.parse::<u32>().map_or_else(
                                |_| {
                                    warn!(
                                        "Failed to parse line {} into a post id",
                                        line.bright_blue().bold()
                                    );
                                    None
                                },
                                Some,
                            )
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
            #[cfg(feature = "e621")]
            ImageBoards::E621 => {
                let mut unit = PostExtractor::new(
                    &[""],
                    &[],
                    true,
                    true,
                    E621Api::new(),
                    args.imageboard.clone(),
                );

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
                            line.parse::<u32>().map_or_else(
                                |_| {
                                    warn!(
                                        "Failed to parse line {} into a post id",
                                        line.bright_blue().bold()
                                    );
                                    None
                                },
                                Some,
                            )
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
            #[cfg(feature = "gelbooru")]
            ImageBoards::GelbooruV0_2 | ImageBoards::Gelbooru => {
                let unit = PostExtractor::new(
                    &[""],
                    &[],
                    true,
                    true,
                    GelbooruApi::new(),
                    args.imageboard.clone(),
                );

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
                            line.parse::<u32>().map_or_else(
                                |_| {
                                    warn!(
                                        "Failed to parse line {} into a post id",
                                        line.bright_blue().bold()
                                    );
                                    None
                                },
                                Some,
                            )
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
            #[cfg(feature = "moebooru")]
            ImageBoards::Moebooru => Err(CliError::ExtractorUnsupportedMode),

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
