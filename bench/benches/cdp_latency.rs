//! Benchmark: raw CDP command round-trip latency.
//!
//! Placeholder — requires a running Chrome instance.
//! Will be filled in when chromiumoxide integration is complete.

use criterion::{criterion_group, criterion_main, Criterion};

fn bench_cdp_placeholder(c: &mut Criterion) {
    c.bench_function("cdp_roundtrip_placeholder", |b| {
        b.iter(|| {
            // TODO: Connect to Chrome, send Runtime.evaluate('1+1'), measure round-trip
            std::thread::sleep(std::time::Duration::from_nanos(100));
        })
    });
}

criterion_group!(benches, bench_cdp_placeholder);
criterion_main!(benches);
