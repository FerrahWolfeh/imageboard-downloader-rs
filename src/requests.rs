use crate::{DanbooruPost, DanbooruPostCount};
use anyhow::{bail, Error};
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget};
use log::debug;

use reqwest::Client;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

use crate::progress_bar::{download_progress_style, master_progress_style};

const DANBOORU_COUNT: &str = "https://danbooru.donmai.us/counts/posts.json?tags=";

pub async fn fetch_items(client: &Client, tag: String) -> Result<Vec<DanbooruPost>, Error> {
    let count = client
        .get(format!("{}{}", DANBOORU_COUNT, tag))
        .send()
        .await?
        .json::<DanbooruPostCount>()
        .await?;

    let total = (count.counts.posts / 200.0).ceil() as u64;

    debug!("{} Posts", count.counts.posts);
    debug!("{} Pages", total);

    let pb = ProgressBar::new(total).with_style(master_progress_style());
    pb.set_draw_target(ProgressDrawTarget::stderr_with_hz(60));
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.println("Scanning Items");

    let mut good_results: Vec<DanbooruPost> = Vec::new();

    for i in 1..=total {
        let jj = client
            .get(format!(
                "https://danbooru.donmai.us/posts.json?tags={}&page={}&limit=200",
                tag, i
            ))
            .send()
            .await?
            .json::<Vec<DanbooruPost>>()
            .await?;
        for i in jj {
            match i.file_url {
                None => {}
                Some(_) => good_results.push(i),
            }
        }
        pb.inc(1);
    }
    debug!("{} collected posts", good_results.len());
    pb.finish_and_clear();
    Ok(good_results)
}

pub async fn download_all(
    client: &Client,
    items: &[DanbooruPost],
    temp_path: &Path,
) -> Result<(), Error> {
    let bar = ProgressBar::new(items.len() as u64).with_style(master_progress_style());
    bar.set_draw_target(ProgressDrawTarget::stderr_with_hz(60));
    bar.enable_steady_tick(Duration::from_millis(100));

    let multi = Arc::new(MultiProgress::new());
    let main = Arc::new(multi.add(bar));

    futures::stream::iter(items)
        .map(|d| fetch(client, d, multi.clone(), main.clone(), temp_path))
        .buffer_unordered(3)
        .collect::<Vec<_>>()
        .await;

    main.finish_and_clear();
    Ok(())
}

pub async fn fetch(
    client: &Client,
    url: &DanbooruPost,
    multi: Arc<MultiProgress>,
    main: Arc<ProgressBar>,
    tmp_dir: &Path,
) -> Result<(), Error> {
    debug!("Fetching {}", &url.file_url.clone().unwrap());
    let res = client.get(url.file_url.clone().unwrap()).send().await?;

    let size = res.content_length().unwrap_or_default();
    let bar = ProgressBar::new(size).with_style(download_progress_style());
    bar.set_draw_target(ProgressDrawTarget::stderr_with_hz(60));

    let pb = multi.add(bar);

    let output = tmp_dir.join(format!(
        "{}.{}",
        url.md5.clone().unwrap(),
        url.file_ext.clone().unwrap()
    ));

    debug!("Creating destination file {:?}", &output);
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(output)
        .await?;

    // Download the file chunk by chunk.
    debug!("Retrieving chunks...");
    let mut stream = res.bytes_stream();
    while let Some(item) = stream.next().await {
        // Retrieve chunk.
        let mut chunk = match item {
            Ok(chunk) => chunk,
            Err(e) => {
                bail!(e)
            }
        };
        pb.inc(chunk.len() as u64);

        // Write to file.
        match file.write_all_buf(&mut chunk).await {
            Ok(_res) => (),
            Err(e) => {
                bail!(e);
            }
        };
    }

    pb.finish_and_clear();

    main.inc(1);
    Ok(())
}
