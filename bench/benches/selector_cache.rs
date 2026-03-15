//! Benchmark: selector cache performance.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rayo_core::selector_cache::SelectorCache;

fn bench_cache_hit(c: &mut Criterion) {
    let mut cache = SelectorCache::new(1000);
    cache.put("div.target".into(), "obj-123".into());

    c.bench_function("selector_cache_hit", |b| {
        b.iter(|| {
            black_box(cache.get("div.target"));
        })
    });
}

fn bench_cache_miss(c: &mut Criterion) {
    let mut cache = SelectorCache::new(1000);
    cache.put("div.target".into(), "obj-123".into());
    cache.invalidate(); // Force miss

    c.bench_function("selector_cache_miss", |b| {
        b.iter(|| {
            black_box(cache.get("div.target"));
        })
    });
}

fn bench_cache_put(c: &mut Criterion) {
    let mut cache = SelectorCache::new(1000);

    c.bench_function("selector_cache_put", |b| {
        let mut i = 0u64;
        b.iter(|| {
            cache.put(format!("div.item-{i}"), format!("obj-{i}"));
            i += 1;
        })
    });
}

criterion_group!(benches, bench_cache_hit, bench_cache_miss, bench_cache_put);
criterion_main!(benches);
