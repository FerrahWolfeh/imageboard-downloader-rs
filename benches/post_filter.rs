use ahash::AHashSet;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use imageboard_downloader::{imageboards::post::rating::Rating, Post};
use rand::{
    distributions::{Alphanumeric, DistString},
    seq::SliceRandom,
    thread_rng, Rng,
};

const TAGS: [&str; 27] = [
    "dog",
    "cat",
    "anthro",
    "gore",
    "male",
    "female",
    "skadi_(arknights)",
    "colored_nails",
    "claws",
    "abs",
    "shirt",
    "sex",
    "tall",
    "abstract",
    "pokemon",
    "human",
    "wolf",
    "fox",
    "cervid",
    "deer",
    "whale",
    "helicopter",
    "sword",
    "gun",
    "blood",
    "painting",
    "breasts",
];

const EXTENSIONS: [&str; 5] = ["webm", "jpg", "png", "webp", "avif"];

const RATINGS: [Rating; 4] = [
    Rating::Safe,
    Rating::Questionable,
    Rating::Explicit,
    Rating::Unknown,
];

fn seed_data(num: u64) -> (Vec<Post>, AHashSet<String>) {
    let mut rng = thread_rng();

    let rnum = rng.gen_range(1..=27);

    let random_tlist: AHashSet<String> = TAGS
        .choose_multiple(&mut rng, rnum)
        .map(|t| t.to_string())
        .collect();

    let mut v2: Vec<Post> = vec![];

    for _i in 0..=num {
        let rn = rng.gen_range(0..=27);

        let id = rng.gen_range(1..u64::MAX);

        let md5 = Alphanumeric.sample_string(&mut rng, 32);

        let ext = EXTENSIONS.choose(&mut rng).unwrap().to_string();

        let tags: AHashSet<String> = TAGS
            .choose_multiple(&mut rng, rn)
            .map(|t| t.to_string())
            .collect();

        let rating = RATINGS.choose(&mut rng).unwrap();

        let pst = Post {
            id,
            url: "".to_string(),
            md5,
            extension: ext,
            rating: rating.clone(),
            tags,
        };

        v2.push(pst)
    }
    (v2, random_tlist)
}

pub fn blacklist_filter(list: Vec<Post>, tags: &AHashSet<String>, safe: bool) -> u64 {
    let mut lst = list;
    let original_size = lst.len();
    let blacklist = tags;
    let mut removed = 0;

    if safe {
        lst.retain(|c| c.rating != Rating::Safe);

        let safe_counter = original_size - lst.len();

        removed += safe_counter as u64;
    }

    if !blacklist.is_empty() {
        let secondary_sz = lst.len();
        lst.retain(|c| !c.tags.iter().any(|s| blacklist.contains(s)));

        let bp = secondary_sz - lst.len();
        removed += bp as u64;
    }

    removed
}

fn post_filter_bench(c: &mut Criterion) {
    c.bench_function("Filter 20 Posts", |b| {
        b.iter(|| {
            let (list, tlist) = black_box(seed_data(20));
            blacklist_filter(list, &tlist, false);
        })
    });
    c.bench_function("Filter 50 Posts", |b| {
        b.iter(|| {
            let (list, tlist) = black_box(seed_data(50));
            blacklist_filter(list, &tlist, false);
        })
    });
    c.bench_function("Filter 100 Posts", |b| {
        b.iter(|| {
            let (list, tlist) = black_box(seed_data(100));
            blacklist_filter(list, &tlist, false);
        })
    });
    c.bench_function("Filter 1000 Posts", |b| {
        b.iter(|| {
            let (list, tlist) = black_box(seed_data(1000));
            blacklist_filter(list, &tlist, false);
        })
    });
    c.bench_function("Filter 10000 Posts", |b| {
        b.iter(|| {
            let (list, tlist) = black_box(seed_data(10000));
            blacklist_filter(list, &tlist, false);
        })
    });
}

criterion_group!(benches, post_filter_bench);
criterion_main!(benches);
