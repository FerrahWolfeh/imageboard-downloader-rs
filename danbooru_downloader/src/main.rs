use crate::imageboards::danbooru::DanbooruDownloader;
use anyhow::Error;

extern crate tokio;

mod progress_bars;
mod imageboards;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::builder().format_timestamp(None).init();
    let tag = vec!["pozyomka_(arknights)".to_string()];

    if let Ok(mut dl) = DanbooruDownloader::new(&tag, None, 3).await {
        dl.download().await?;
        println!("{:?}", dl);
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
