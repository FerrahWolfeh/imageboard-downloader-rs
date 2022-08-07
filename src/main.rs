use crate::model_structs::{DanbooruPost, DanbooruPostCount, SAFItemMetadata, SAFMetadata};

mod model_structs;

const DANBOORU_COUNT: &str = "https://danbooru.donmai.us/counts/posts.json?tags=";

fn main() {
    let tag = "kroos_(arknights)";

    let count = reqwest::blocking::get(format!("{}{}", DANBOORU_COUNT, tag))
        .unwrap()
        .json::<DanbooruPostCount>()
        .unwrap();

    let total = (count.counts.posts / 200.0).ceil() as u64;

    println!("{} Posts\n{} Pages", count.counts.posts, total);

    let mut good_results: Vec<DanbooruPost> = Vec::new();

    for i in 1..=total {
        let jj = reqwest::blocking::get(
            format!("https://danbooru.donmai.us/posts.json?tags={}&page={}&limit=200","kroos_(arknights)", i),
        )
        .unwrap()
        .json::<Vec<Option<DanbooruPost>>>()
        .unwrap();
        for i in jj.into_iter().flatten() {
            match i.file_url {
                None => {},
                Some(_) => good_results.push(i),
            }
        }
    }

    let mut saf_items: Vec<SAFItemMetadata> = Vec::new();

    for i in good_results {
        let item = SAFItemMetadata {
            file: i.file_url.unwrap(),
            file_size: i.file_size.unwrap(),
            md5: i.md5.unwrap(),
            sha256: "EEE".to_string()
        };
        saf_items.push(item)
    }

    let saf_hdr = SAFMetadata { item_count: saf_items.len(), item_list: saf_items };

    let encoded = bincode::serialize(&saf_hdr).unwrap();
    let compressed = zstd::encode_all(encoded.as_slice(), 7).unwrap();

    println!("{} collected posts", saf_hdr.item_count);
    println!("{}\n{}", encoded.len(), compressed.len())

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
