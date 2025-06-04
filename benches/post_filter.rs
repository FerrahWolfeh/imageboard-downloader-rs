use ahash::AHashSet;
use criterion::{Criterion, criterion_group, criterion_main};
use ibdl_common::post::{
    Post,
    extension::Extension,
    rating::Rating,
    tags::{Tag, TagType},
};
use rand::{
    Rng,
    distr::{Alphanumeric, SampleString},
    rng,
    seq::IndexedRandom,
};
use std::hint::black_box;

const TAGS: [&str; 135] = [
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
    "convenience_store",
    "black_jacket",
    "full_body",
    "long_hair",
    "viol3t",
    "wolf_girl",
    "wolf_ears",
    "alternate_costume",
    "pink_hair",
    "shoes",
    "hand_in_pocket",
    "solo",
    "holding",
    "tail",
    "night",
    "shelf",
    "texas_(arknights)",
    "wolf_tail",
    "shadow",
    "long_sleeves",
    "absurdres",
    "colored_inner_hair",
    "orange_eyes",
    "pants",
    "indoors",
    "holding_bag",
    "standing",
    "1girl",
    "black_hair",
    "shop",
    "arknights",
    "bag",
    "sidelocks",
    "two-tone_hair",
    "hair_between_eyes",
    "black_pants",
    "black_footwear",
    "animal_ears",
    "jacket",
    "highres",
    "multicolored_hair",
    "heart",
    "original",
    "white_socks",
    "bow",
    "fingernails",
    "holding_carrot",
    "1girl",
    "blue_eyes",
    "eyelashes",
    "heart_background",
    "large_bow",
    "back_bow",
    "open_mouth",
    "vegetable",
    "blue_flower",
    "holding_food",
    "hair_bow",
    "puffy_sleeves",
    "blush",
    "patterned_background",
    "white_pupils",
    "blonde_hair",
    "flower",
    "socks",
    "solo",
    "holding_vegetable",
    "suggestive_fluid",
    "tongue",
    "moyori",
    "bright_pupils",
    "patterned",
    "carrot",
    "food",
    "long_hair",
    "brown_bow",
    "holding",
    "purple_flower",
    "red_flower",
    "white_bow",
    "pointy_ears",
    "long_hair",
    "silver_hair",
    "black_flower",
    "solo",
    "ribbon",
    "cosmicsnic",
    "detached_collar",
    "satella_(re:zero)",
    "black_rose",
    "looking_at_viewer",
    "hand_on_own_cheek",
    "bare_shoulders",
    "black_dress",
    "purple_eyes",
    "hand_on_own_face",
    "rose",
    "flower",
    "highres",
    "re:zero_kara_hajimeru_isekai_seikatsu",
    "dress",
    "hair_ornament",
    "1girl",
    "bad_id",
    "hair_ribbon",
    "black_legwear",
    "bad_pixiv_id",
    "hair_flower",
];

const EXTENSIONS: [&str; 5] = ["webm", "jpg", "png", "webp", "avif"];

const RATINGS: [Rating; 4] = [
    Rating::Safe,
    Rating::Questionable,
    Rating::Explicit,
    Rating::Unknown,
];

fn seed_data(num: u64) -> (Vec<Post>, AHashSet<String>) {
    let mut rng = rng();

    let rnum = rng.random_range(1..=27);

    let random_tlist: AHashSet<String> = TAGS
        .choose_multiple(&mut rng, rnum)
        .map(|t| t.to_string())
        .collect();

    let mut v2: Vec<Post> = vec![];

    for _i in 0..=num {
        let rn = rng.random_range(0..=27);

        let id = rng.random_range(1..u64::MAX);

        let md5 = Alphanumeric.sample_string(&mut rng, 32);

        let ext = EXTENSIONS.choose(&mut rng).unwrap().to_string();

        let tags = TAGS
            .choose_multiple(&mut rng, rn)
            .map(|t| Tag::new(t, TagType::General))
            .collect();

        let rating = *RATINGS.choose(&mut rng).unwrap();

        let pst = Post {
            id,
            website: ibdl_common::ImageBoards::Danbooru,
            url: "".to_string(),
            md5,
            extension: Extension::guess_format(&ext),
            rating,
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
        lst.retain(|c| !c.tags.iter().any(|s| blacklist.contains(&s.tag())));

        let bp = secondary_sz - lst.len();
        removed += bp as u64;
    }

    removed
}

fn post_filter_bench(c: &mut Criterion) {
    c.bench_function("Filter 20 Posts", |b| {
        let (list, tlist) = seed_data(20);
        b.iter(|| {
            black_box(blacklist_filter(list.clone(), &tlist, false));
        })
    });
    c.bench_function("Filter 50 Posts", |b| {
        let (list, tlist) = seed_data(100);
        b.iter(|| {
            black_box(blacklist_filter(list.clone(), &tlist, false));
        })
    });
    c.bench_function("Filter 100 Posts", |b| {
        let (list, tlist) = seed_data(100);
        b.iter(|| {
            black_box(blacklist_filter(list.clone(), &tlist, false));
        })
    });
    c.bench_function("Filter 1000 Posts", |b| {
        let (list, tlist) = seed_data(1000);
        b.iter(|| {
            black_box(blacklist_filter(list.clone(), &tlist, false));
        })
    });
    c.bench_function("Filter 10000 Posts", |b| {
        let (list, tlist) = seed_data(10000);
        b.iter(|| {
            black_box(blacklist_filter(list.clone(), &tlist, false));
        })
    });
}

criterion_group!(benches, post_filter_bench);
criterion_main!(benches);
