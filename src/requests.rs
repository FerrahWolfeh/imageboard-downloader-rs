use std::path::Path;
use crate::{DanbooruPost, DanbooruPostCount};
use anyhow::Error;
use futures::StreamExt;
use log::debug;
use md5::compute;
use reqwest::Client;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

const DANBOORU_COUNT: &str = "https://danbooru.donmai.us/counts/posts.json?tags=";

pub async fn fetch_items(client: &Client, tag: String) -> Result<Vec<DanbooruPost>, Error> {

    //let client = Client::builder().user_agent("LibSFA 0.1 - testing").build()?;

    let count = client.get(format!("{}{}", DANBOORU_COUNT, tag)).send().await?.json::<DanbooruPostCount>().await?;

    let total = (count.counts.posts / 200.0).ceil() as u64;

    debug!("{} Posts", count.counts.posts);
    debug!("{} Pages", total);

    let mut good_results: Vec<DanbooruPost> = Vec::new();

    for i in 1..=total {
        let jj = client.get(format!(
            "https://danbooru.donmai.us/posts.json?tags={}&page={}&limit=200",
            tag, i
        ))
            .send().await?
            .json::<Vec<DanbooruPost>>()
            .await?;
        for i in jj {
            match i.file_url {
                None => {}
                Some(_) => good_results.push(i),
            }
        }
    }
    debug!("{} collected posts", good_results.len());
    Ok(good_results)
}

pub async fn download_all(client: &Client, items: &[DanbooruPost], temp_path: &Path) -> Result<(), Error> {
    let fetches = futures::stream::iter(
        items.iter().map(|path| {
            async move {
                let lp = path.file_ext.clone().unwrap();
                let link = path.file_url.clone().unwrap();
                let file = client.get(&link).send().await.unwrap().bytes().await.unwrap();
                File::create(temp_path.join(format!("{:x}.{}", compute(&file), lp))).await.unwrap().write_all(&file).await.unwrap();


            }
        })
    ).buffer_unordered(3).collect::<Vec<()>>();
    fetches.await;
    Ok(())
}