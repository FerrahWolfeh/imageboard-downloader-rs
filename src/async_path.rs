use std::sync::{atomic::AtomicU64, Arc};

use color_eyre::eyre::Result;
use ibdl_common::{
    post::PostQueue,
    reqwest::Client,
    tokio::{join, sync::mpsc::unbounded_channel},
    ImageBoards,
};
use ibdl_core::{cli::Cli, queue::Queue};
use ibdl_extractors::websites::{danbooru::DanbooruExtractor, AsyncFetch, Extractor};
use once_cell::sync::Lazy;

use crate::utils::auth_imgboard;

static POST_COUNTER: Lazy<Arc<AtomicU64>> = Lazy::new(|| Arc::new(AtomicU64::new(0)));

pub async fn async_path(args: &Cli) -> Result<()> {
    let nt = args.name_type();

    let (ext, client) = search_args_async(args).await?;

    let dirname = args.generate_save_path()?;

    let qw = Queue::new(
        *args.imageboard,
        PostQueue {
            imageboard: *args.imageboard,
            client: client.clone(),
            posts: vec![],
            tags: vec![],
        },
        args.simultaneous_downloads,
        Some(client),
        args.cbz,
    );

    let (channel_tx, channel_rx) = unbounded_channel();

    let ext_thd = ext.setup_fetch_thread(
        channel_tx,
        args.start_page,
        args.limit,
        Some(POST_COUNTER.clone()),
    );
    let asd =
        qw.setup_async_downloader(channel_rx, dirname, nt, args.annotate, POST_COUNTER.clone());

    let (_, results) = join!(ext_thd, asd);

    results??;

    Ok(())
}

async fn search_args_async(args: &Cli) -> Result<(impl AsyncFetch, Client)> {
    let ratings = args.selected_ratings();

    match *args.imageboard {
        ImageBoards::Danbooru => {
            let mut unit = DanbooruExtractor::new(&args.tags, &ratings, args.disable_blacklist);
            auth_imgboard(args.auth, &mut unit).await?;

            let extractor = unit.clone();

            let client = unit.client();

            Ok((extractor, client))
        }
        _ => unimplemented!(),
    }
}
