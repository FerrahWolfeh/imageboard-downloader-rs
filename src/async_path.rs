use std::sync::{atomic::AtomicU64, Arc};

use color_eyre::eyre::Result;
use ibdl_common::{
    post::Post,
    reqwest::Client,
    tokio::{
        join,
        sync::mpsc::{unbounded_channel, UnboundedSender},
        task::JoinHandle,
    },
    ImageBoards,
};
use ibdl_core::{async_queue::Queue, cli::Cli};
use ibdl_extractors::{
    error::ExtractorError,
    websites::{
        danbooru::DanbooruExtractor, e621::E621Extractor, gelbooru::GelbooruExtractor,
        moebooru::MoebooruExtractor, AsyncFetch, Extractor, MultiWebsite,
    },
};
use once_cell::sync::Lazy;

use crate::utils::{auth_imgboard, print_results};

static POST_COUNTER: Lazy<Arc<AtomicU64>> = Lazy::new(|| Arc::new(AtomicU64::new(0)));

pub async fn async_path(args: &Cli) -> Result<()> {
    let (channel_tx, channel_rx) = unbounded_channel();

    let (ext, client) = search_args_async(args, channel_tx).await?;

    let dirname = args.generate_save_path()?;

    let qw = Queue::new(
        *args.imageboard,
        args.simultaneous_downloads,
        Some(client),
        args.cbz,
    );

    let asd = qw.setup_async_downloader(
        channel_rx,
        dirname,
        args.name_type(),
        args.annotate,
        POST_COUNTER.clone(),
    );

    let (removed, results) = join!(ext, asd);

    print_results(results??, removed??);

    Ok(())
}

async fn search_args_async(
    args: &Cli,
    channel_tx: UnboundedSender<Post>,
) -> Result<(JoinHandle<Result<u64, ExtractorError>>, Client)> {
    let ratings = args.selected_ratings();

    match *args.imageboard {
        ImageBoards::Danbooru => {
            let mut unit = DanbooruExtractor::new(&args.tags, &ratings, args.disable_blacklist);
            auth_imgboard(args.auth, &mut unit).await?;

            let client = unit.client();

            let ext_thd = unit.setup_fetch_thread(
                channel_tx,
                args.start_page,
                args.limit,
                Some(POST_COUNTER.clone()),
            );

            Ok((ext_thd, client))
        }
        ImageBoards::E621 => {
            let mut unit = E621Extractor::new(&args.tags, &ratings, args.disable_blacklist);
            auth_imgboard(args.auth, &mut unit).await?;

            let client = unit.client();

            let ext_thd = unit.setup_fetch_thread(
                channel_tx,
                args.start_page,
                args.limit,
                Some(POST_COUNTER.clone()),
            );

            Ok((ext_thd, client))
        }
        ImageBoards::Rule34 | ImageBoards::Realbooru | ImageBoards::Gelbooru => {
            let unit = GelbooruExtractor::new(&args.tags, &ratings, args.disable_blacklist)
                .set_imageboard(*args.imageboard)?;
            let client = unit.client();

            let ext_thd = unit.setup_fetch_thread(
                channel_tx,
                args.start_page,
                args.limit,
                Some(POST_COUNTER.clone()),
            );

            Ok((ext_thd, client))
        }
        ImageBoards::Konachan => {
            let unit = MoebooruExtractor::new(&args.tags, &ratings, args.disable_blacklist);
            let client = unit.client();

            let ext_thd = unit.setup_fetch_thread(
                channel_tx,
                args.start_page,
                args.limit,
                Some(POST_COUNTER.clone()),
            );

            Ok((ext_thd, client))
        }
    }
}
