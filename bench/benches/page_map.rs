//! Benchmark: page map generation and serialization.

use std::time::Duration;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rayo_core::page_map::{InteractiveElement, PageMap};

fn configured() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::from_secs(3))
        .warm_up_time(Duration::from_secs(1))
        .noise_threshold(0.05)
}

fn make_page_map(n_elements: usize) -> PageMap {
    let interactive: Vec<InteractiveElement> = (0..n_elements)
        .map(|i| InteractiveElement {
            id: i,
            tag: "input".into(),
            r#type: Some("text".into()),
            name: Some(format!("field_{i}")),
            label: Some(format!("Field {i}")),
            text: None,
            placeholder: Some(format!("Enter field {i}...")),
            value: None,
            options: None,
            role: None,
            href: None,
            selector: format!("input[name='field_{i}']"),
        })
        .collect();

    PageMap {
        url: "https://example.com/form".into(),
        title: "Test Form".into(),
        interactive,
        headings: vec!["Test Form".into(), "Personal Info".into()],
        text_summary: "A test form with many fields.".into(),
    }
}

fn bench_page_map_serialize(c: &mut Criterion) {
    let map = make_page_map(20);

    c.bench_function("page_map_serialize_20_elements", |b| {
        b.iter(|| {
            black_box(serde_json::to_string(&map).unwrap());
        })
    });
}

fn bench_page_map_token_estimate(c: &mut Criterion) {
    let map = make_page_map(20);

    c.bench_function("page_map_token_estimate", |b| {
        b.iter(|| {
            black_box(map.estimated_tokens());
        })
    });
}

fn bench_page_map_large(c: &mut Criterion) {
    let map = make_page_map(100);

    c.bench_function("page_map_serialize_100_elements", |b| {
        b.iter(|| {
            black_box(serde_json::to_string(&map).unwrap());
        })
    });
}

criterion_group! {
    name = benches;
    config = configured();
    targets = bench_page_map_serialize, bench_page_map_token_estimate, bench_page_map_large
}
criterion_main!(benches);
