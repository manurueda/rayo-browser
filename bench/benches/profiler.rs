//! Benchmark: profiler overhead measurement.
//!
//! The profiler is on by default, so its cost is added to every operation.
//! This benchmark quantifies that cost: mutex lock + ring buffer write per span.

use std::time::Duration;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rayo_profiler::{Profiler, SpanCategory};

fn configured() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::from_secs(3))
        .warm_up_time(Duration::from_secs(1))
        .noise_threshold(0.05)
}

fn bench_start_span_drop(c: &mut Criterion) {
    let profiler = Profiler::new();

    c.bench_function("profiler_start_span_drop", |b| {
        b.iter(|| {
            let _span =
                profiler.start_span(black_box("test_operation"), black_box(SpanCategory::User));
            // span dropped here — records duration
        })
    });
}

fn bench_nested_spans(c: &mut Criterion) {
    let profiler = Profiler::new();

    c.bench_function("profiler_two_nested_spans", |b| {
        b.iter(|| {
            let _parent = profiler.start_span("parent", SpanCategory::Batch);
            let _child = profiler.start_span("child", SpanCategory::CdpCommand);
            // both dropped here
        })
    });
}

fn bench_profiler_heavy_load(c: &mut Criterion) {
    let profiler = Profiler::new();

    c.bench_function("profiler_10_spans_batch", |b| {
        b.iter(|| {
            // Simulate a batch of 10 actions, each with a profiler span
            for i in 0..10 {
                let _span =
                    profiler.start_span(black_box("action"), black_box(SpanCategory::DomMutate));
                black_box(i);
            }
        })
    });
}

fn bench_profiler_span_count(c: &mut Criterion) {
    let profiler = Profiler::new();
    // Pre-fill with 10k spans
    for _ in 0..10_000 {
        let _span = profiler.start_span("warmup", SpanCategory::User);
    }

    c.bench_function("profiler_span_count_10k", |b| {
        b.iter(|| {
            black_box(profiler.span_count());
        })
    });
}

criterion_group! {
    name = benches;
    config = configured();
    targets = bench_start_span_drop, bench_nested_spans, bench_profiler_heavy_load, bench_profiler_span_count
}
criterion_main!(benches);
