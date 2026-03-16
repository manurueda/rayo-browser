//! Benchmark: selector cache performance.

use std::time::Duration;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rayo_core::selector_cache::SelectorCache;

fn configured() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::from_secs(3))
        .warm_up_time(Duration::from_secs(1))
        .noise_threshold(0.05)
}

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

fn bench_cache_churn(c: &mut Criterion) {
    let mut cache = SelectorCache::new(100);
    // Pre-fill to capacity
    for i in 0..100 {
        cache.put(format!("div.item-{i}"), format!("obj-{i}"));
    }

    c.bench_function("selector_cache_churn_at_capacity", |b| {
        let mut i = 100u64;
        b.iter(|| {
            // Evict oldest, insert new — simulates real batch workloads
            cache.put(format!("div.item-{i}"), format!("obj-{i}"));
            i += 1;
        })
    });
}

fn bench_cache_invalidate_and_refill(c: &mut Criterion) {
    let mut cache = SelectorCache::new(1000);

    c.bench_function("selector_cache_invalidate_refill_20", |b| {
        b.iter(|| {
            // Simulate: DOM mutation → invalidate → re-resolve 20 selectors
            cache.invalidate();
            for i in 0..20 {
                cache.put(format!("input[name='field_{i}']"), format!("obj-{i}"));
            }
            black_box(cache.get("input[name='field_10']"));
        })
    });
}

criterion_group! {
    name = benches;
    config = configured();
    targets = bench_cache_hit, bench_cache_miss, bench_cache_put, bench_cache_churn, bench_cache_invalidate_and_refill
}
criterion_main!(benches);
