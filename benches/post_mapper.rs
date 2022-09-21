use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ibdl_common::post::rating::Rating;
use ibdl_extractors::websites::e621::E621Extractor;
use ibdl_extractors::websites::Extractor;

static TEST_JSON_E621: &str = include_str!("../assets/test_list_e621.json");

fn post_mapper_e621(c: &mut Criterion) {
    c.bench_function("Map E621 posts", |b| {
        let ee = E621Extractor::new(&["bb"], &[Rating::Safe, Rating::Questionable], false);
        b.iter(|| ee.map_posts(black_box(TEST_JSON_E621.to_string())))
    });
}

criterion_group!(benches, post_mapper_e621);
criterion_main!(benches);
