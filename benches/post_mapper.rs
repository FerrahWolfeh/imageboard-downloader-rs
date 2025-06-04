use criterion::{Criterion, criterion_group, criterion_main};
use ibdl_common::post::rating::Rating;
use ibdl_extractors::extractor_config::DEFAULT_SERVERS;
use ibdl_extractors::imageboards::danbooru::DanbooruExtractor;
use ibdl_extractors::imageboards::e621::E621Extractor;
use ibdl_extractors::imageboards::gelbooru::GelbooruExtractor;
use ibdl_extractors::imageboards::moebooru::MoebooruExtractor;
use ibdl_extractors::prelude::Extractor;
use std::hint::black_box;

static TEST_JSON_E621: &str = include_str!("../assets/sample_post_lists/test_list_e621.json");
static TEST_JSON_DANBOORU: &str =
    include_str!("../assets/sample_post_lists/test_list_danbooru.json");
static TEST_JSON_KONACHAN: &str =
    include_str!("../assets/sample_post_lists/test_list_konachan.json");
static TEST_JSON_R34: &str = include_str!("../assets/sample_post_lists/test_list_r34.json");
static TEST_JSON_GB: &str = include_str!("../assets/sample_post_lists/test_list_gb.json");
static TEST_JSON_RB: &str = include_str!("../assets/sample_post_lists/test_list_rb.json");

fn post_mapper_e621(c: &mut Criterion) {
    c.bench_function("Map 200 E621 posts", |b| {
        let ee = E621Extractor::new(
            black_box(&["bb"]),
            &[Rating::Safe, Rating::Questionable],
            false,
            true,
        );
        b.iter(|| black_box(ee.map_posts(black_box(TEST_JSON_E621.to_string()))))
    });
    c.bench_function("Map 200 Danbooru posts", |b| {
        let ee = DanbooruExtractor::new(
            black_box(&["bb"]),
            &[Rating::Safe, Rating::Questionable],
            false,
            true,
        );
        b.iter(|| black_box(ee.map_posts(black_box(TEST_JSON_DANBOORU.to_string()))))
    });
    c.bench_function("Map 200 Konachan posts", |b| {
        let ee = MoebooruExtractor::new(
            black_box(&["bb"]),
            &[Rating::Safe, Rating::Questionable],
            false,
            true,
        );
        b.iter(|| black_box(ee.map_posts(black_box(TEST_JSON_KONACHAN.to_string()))))
    });
    c.bench_function("Map 200 Rule34 posts", |b| {
        let ee = GelbooruExtractor::new_with_config(
            black_box(&["bb"]),
            &[Rating::Safe, Rating::Questionable],
            false,
            true,
            DEFAULT_SERVERS.get("rule34").unwrap().clone(),
        );
        b.iter(|| black_box(ee.map_posts(black_box(TEST_JSON_R34.to_string()))))
    });
    c.bench_function("Map 200 Gelbooru posts", |b| {
        let ee = GelbooruExtractor::new_with_config(
            black_box(&["bb"]),
            &[Rating::Safe, Rating::Questionable],
            false,
            true,
            DEFAULT_SERVERS.get("gelbooru").unwrap().clone(),
        );
        b.iter(|| black_box(ee.map_posts(black_box(TEST_JSON_GB.to_string()))))
    });
    c.bench_function("Map 200 Realbooru posts", |b| {
        let ee = GelbooruExtractor::new_with_config(
            black_box(&["bb"]),
            &[Rating::Safe, Rating::Questionable],
            false,
            true,
            DEFAULT_SERVERS.get("realbooru").unwrap().clone(),
        );
        b.iter(|| black_box(ee.map_posts(black_box(TEST_JSON_RB.to_string()))))
    });
}

criterion_group!(benches, post_mapper_e621);
criterion_main!(benches);
