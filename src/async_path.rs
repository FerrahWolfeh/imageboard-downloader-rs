use std::sync::{atomic::AtomicU64, Arc};

use color_eyre::eyre::{bail, Result};
use ibdl_common::{
    post::Post,
    reqwest::Client,
    tokio::{
        join,
        sync::mpsc::{channel, unbounded_channel, Sender, UnboundedSender},
    },
    ImageBoards,
};
use ibdl_core::{async_queue::Queue, cli::Cli};
use ibdl_extractors::websites::{
    danbooru::DanbooruExtractor, e621::E621Extractor, gelbooru::GelbooruExtractor,
    moebooru::MoebooruExtractor, AsyncFetch, Extractor, ExtractorThreadHandle, MultiWebsite,
    PoolExtract,
};
use once_cell::sync::Lazy;

use crate::utils::{auth_imgboard, print_results};

static POST_COUNTER: Lazy<Arc<AtomicU64>> = Lazy::new(|| Arc::new(AtomicU64::new(0)));

pub async fn async_path(args: &Cli) -> Result<()> {
    let (channel_tx, channel_rx) = unbounded_channel();

    let (length_sender, length_channel) = channel(8);

    let (ext, client) = search_args_async(args, channel_tx, length_sender).await?;

    let dirname = args.generate_save_path()?;

    let qw = Queue::new(
        *args.imageboard,
        args.simultaneous_downloads,
        Some(client),
        args.cbz,
        args.pool_id.is_some(),
        args.name_type(),
        args.annotate,
    );

    let asd = qw.setup_async_downloader(dirname, POST_COUNTER.clone(), channel_rx, length_channel);

    let (Ok(removed), Ok(results)) = join!(ext, asd) else {bail!("Failed starting threads!")};

    print_results(results?, removed?);

    Ok(())
}

async fn search_args_async(
    args: &Cli,
    channel_tx: UnboundedSender<Post>,
    length_tx: Sender<u64>,
) -> Result<(ExtractorThreadHandle, Client)> {
    let ratings = args.selected_ratings();

    match *args.imageboard {
        ImageBoards::Danbooru => {
            let mut unit = DanbooruExtractor::new(
                &args.tags,
                &ratings,
                args.disable_blacklist,
                !args.no_animated,
            );
            auth_imgboard(args.auth, &mut unit).await?;

            unit.exclude_tags(&args.exclude);

            if let Some(ext) = args.get_extension() {
                unit.force_extension(ext);
            }

            if args.pool_id.is_some() {
                unit.setup_pool_download(args.pool_id)
            }

            let client = unit.client();

            let ext_thd =
                unit.setup_fetch_thread(channel_tx, args.start_page, args.limit, Some(length_tx));

            Ok((ext_thd, client))
        }
        ImageBoards::E621 => {
            let mut unit = E621Extractor::new(
                &args.tags,
                &ratings,
                args.disable_blacklist,
                !args.no_animated,
            );
            auth_imgboard(args.auth, &mut unit).await?;

            unit.exclude_tags(&args.exclude);

            if let Some(ext) = args.get_extension() {
                unit.force_extension(ext);
            }

            let client = unit.client();

            let ext_thd =
                unit.setup_fetch_thread(channel_tx, args.start_page, args.limit, Some(length_tx));

            Ok((ext_thd, client))
        }
        ImageBoards::Rule34 | ImageBoards::Realbooru | ImageBoards::Gelbooru => {
            let mut unit = GelbooruExtractor::new(
                &args.tags,
                &ratings,
                args.disable_blacklist,
                !args.no_animated,
            );

            unit.exclude_tags(&args.exclude)
                .set_imageboard(*args.imageboard);

            if let Some(ext) = args.get_extension() {
                unit.force_extension(ext);
            }

            let client = unit.client();

            let ext_thd =
                unit.setup_fetch_thread(channel_tx, args.start_page, args.limit, Some(length_tx));

            Ok((ext_thd, client))
        }
        ImageBoards::Konachan => {
            let mut unit = MoebooruExtractor::new(
                &args.tags,
                &ratings,
                args.disable_blacklist,
                !args.no_animated,
            );
            let client = unit.client();

            unit.exclude_tags(&args.exclude);

            if let Some(ext) = args.get_extension() {
                unit.force_extension(ext);
            }

            let ext_thd =
                unit.setup_fetch_thread(channel_tx, args.start_page, args.limit, Some(length_tx));

            Ok((ext_thd, client))
        }
    }
}
