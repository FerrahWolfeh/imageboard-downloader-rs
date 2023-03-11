use color_eyre::eyre::Result;
use ibdl_common::{
    post::{NameType, PostQueue},
    reqwest::Client,
    tokio::{join, sync::mpsc::unbounded_channel},
    ImageBoards,
};
use ibdl_core::{cli::Cli, queue::Queue};
use ibdl_extractors::websites::{danbooru::DanbooruExtractor, AsyncFetch, Extractor};

use crate::utils::{auth_imgboard, convert_rating_list, generate_save_path};

pub async fn async_path(args: &Cli) -> Result<()> {
    let nt = if args.save_file_as_id {
        NameType::ID
    } else {
        NameType::MD5
    };

    let (ext, client) = search_args_async(args).await?;

    let dirname = generate_save_path(args)?;

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

    let ext_thd = ext.setup_fetch_thread(channel_tx, args.start_page, args.limit);
    let asd = qw.setup_async_downloader(channel_rx, dirname, nt, args.annotate);

    let (_, results) = join!(ext_thd, asd);

    results??;

    Ok(())
}

async fn search_args_async(args: &Cli) -> Result<(impl AsyncFetch, Client)> {
    let ratings = convert_rating_list(args);

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
