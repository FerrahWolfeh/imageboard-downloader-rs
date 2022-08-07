use anyhow::Error;
use reqwest::Client;
use tempdir::TempDir;
use crate::model_structs::{DanbooruPost, DanbooruPostCount, SAFItemMetadata, SAFMetadata};
use crate::requests::{download_all, fetch_items};

extern crate tokio;

mod model_structs;
mod requests;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::builder().format_timestamp(None).init();
    let tag = "kroos_(arknights)";
    let client = Client::builder().user_agent("LibSFA 0.1 - testing").build()?;

    let good_results = fetch_items(&client, tag.to_string()).await?;

    let temp = TempDir::new_in("/mnt/ram", "kk")?.into_path();

    download_all(&client, &good_results, &temp).await?;



    let mut saf_items: Vec<SAFItemMetadata> = Vec::new();

    for i in good_results {
        let item = SAFItemMetadata {
            file: i.file_url.unwrap(),
            file_size: i.file_size,
            md5: "FFF".to_string(),
            sha256: "EEE".to_string(),
        };
        saf_items.push(item)
    }

    let saf_hdr = SAFMetadata {
        item_count: saf_items.len(),
        item_list: saf_items,
    };

    let encoded = bincode::serialize(&saf_hdr).unwrap();
    let compressed = zstd::encode_all(encoded.as_slice(), 7).unwrap();

    println!(
        "SAF Header raw size: {} B\nSAF Header compressed size: {} B",
        encoded.len(),
        compressed.len()
    );
    Ok(())

    //

    // let mut good_results: Vec<DanbooruPost> = Vec::new();
    // let mut failed: u64 = 0;
    //
    // for i in jj {
    //     if let Some(item) = i {
    //         match item.file_url {
    //             None =>  {failed += 1;}
    //             Some(_) => {good_results.push(item)}
    //         }
    //     }
    // }

    //println!("{}", serde_json::to_string_pretty(&jj).unwrap())
}
