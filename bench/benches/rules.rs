//! Benchmark: rule engine evaluation cost.
//!
//! The rule engine runs on every tool call (check_selector, check_screenshot,
//! check_budget). This measures the per-action overhead.

use std::time::Duration;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rayo_rules::{RayoRulesConfig, RuleEngine};

fn configured() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::from_secs(3))
        .warm_up_time(Duration::from_secs(1))
        .noise_threshold(0.05)
}

fn bench_check_selector_css(c: &mut Criterion) {
    let config = RayoRulesConfig::default();
    let mut engine = RuleEngine::new(config);

    c.bench_function("rule_check_selector_css", |b| {
        b.iter(|| {
            black_box(engine.check_selector(black_box("#submit-button")));
        })
    });
}

fn bench_check_selector_xpath(c: &mut Criterion) {
    let config = RayoRulesConfig::default();
    let mut engine = RuleEngine::new(config);

    c.bench_function("rule_check_selector_xpath", |b| {
        b.iter(|| {
            black_box(engine.check_selector(black_box("//div[@class='foo']")));
        })
    });
}

fn bench_check_selector_broad(c: &mut Criterion) {
    let config = RayoRulesConfig::default();
    let mut engine = RuleEngine::new(config);

    c.bench_function("rule_check_selector_broad", |b| {
        b.iter(|| {
            black_box(engine.check_selector(black_box("div")));
        })
    });
}

fn bench_check_budget(c: &mut Criterion) {
    let config = RayoRulesConfig::default();
    let mut engine = RuleEngine::new(config);

    c.bench_function("rule_check_budget", |b| {
        b.iter(|| {
            black_box(engine.check_budget(black_box("cdp_command"), black_box(25.0)));
        })
    });
}

fn bench_check_screenshot_rate(c: &mut Criterion) {
    let config = RayoRulesConfig::default();
    let mut engine = RuleEngine::new(config);

    c.bench_function("rule_check_screenshot_rate", |b| {
        b.iter(|| {
            black_box(engine.check_screenshot());
        })
    });
}

criterion_group! {
    name = benches;
    config = configured();
    targets = bench_check_selector_css, bench_check_selector_xpath, bench_check_selector_broad, bench_check_budget, bench_check_screenshot_rate
}
criterion_main!(benches);
