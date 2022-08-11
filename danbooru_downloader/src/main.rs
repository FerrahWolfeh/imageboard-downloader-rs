use std::path::PathBuf;
use crate::imageboards::danbooru::DanbooruDownloader;
use crate::imageboards::ImageBoards;
use anyhow::Error;
use clap::Parser;

extern crate tokio;

mod imageboards;
mod progress_bars;

#[derive(Parser, Debug)]
#[clap(name = "Booru Downloader", author, version, about, long_about = None)]
struct Cli {
    /// Specify imageboard to download from
    //#[clap(default_value_t = ImageBoards::Danbooru, ignore_case = true, possible_values = &["danbooru", "e621", "rule34", "realbooru"])]
    #[clap(short, long, arg_enum, ignore_case = true, default_value_t = ImageBoards::Danbooru)]
    imageboard: ImageBoards,

    /// Output dir
    #[clap(short, parse(from_os_str))]
    output: Option<PathBuf>,

    /// Tags to search
    #[clap(value_parser, required = true)]
    tags: Vec<String>,

    /// Number of simultaneous downloads
    #[clap(short, value_parser, default_value_t = 3)]
    simultaneous_downloads: usize
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Cli::parse();
    env_logger::builder().format_timestamp(None).init();


    if let Ok(mut dl) = DanbooruDownloader::new(&args.tags, args.output, args.simultaneous_downloads).await {
        dl.download().await?;
    } else {
        println!("No posts found for tag selection!")
    }

    // let client = Client::builder()
    //     .user_agent("LibSFA 0.1 - testing")
    //     .build()?;

    // let good_results = fetch_items(&client, tag.to_string()).await?;
    //
    // download_all(&client, &good_results, &temp).await?;
    //
    // let mut saf_items: Vec<SAFItemMetadata> = Vec::new();
    //
    // for i in good_results {
    //     let item = SAFItemMetadata {
    //         file: i.file_url.unwrap(),
    //         file_size: i.file_size,
    //         md5: "FFF".to_string(),
    //         sha256: "EEE".to_string(),
    //     };
    //     saf_items.push(item)
    // }
    //
    // let saf_hdr = SAFMetadata {
    //     item_count: saf_items.len(),
    //     item_list: saf_items,
    // };
    //
    // let encoded = bincode::serialize(&saf_hdr).unwrap();
    // let compressed = zstd::encode_all(encoded.as_slice(), 7).unwrap();
    //
    // println!(
    //     "SAF Header raw size: {} B\nSAF Header compressed size: {} B",
    //     encoded.len(),
    //     compressed.len()
    // );
    Ok(())
}
