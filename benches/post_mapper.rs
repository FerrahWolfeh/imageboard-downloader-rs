use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ibdl_common::post::{rating::Rating, Post};
use rand::{
    distributions::{Alphanumeric, DistString},
    seq::SliceRandom,
    thread_rng, Rng,
};

static TEST_JSON: &str = include_str!("../assets/test_list_e621.json");

fn post_filter_bench(c: &mut Criterion) {
    c.bench_function("Filter 20 Posts", |b| b.iter(|| {}));
}

criterion_group!(benches, post_filter_bench);
criterion_main!(benches);
