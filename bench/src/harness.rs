//! Benchmark harness: warmup, iterations, statistical analysis.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Configuration for a benchmark run.
pub struct BenchConfig {
    pub warmup_iterations: usize,
    pub measure_iterations: usize,
    pub timeout: Duration,
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            warmup_iterations: 5,
            measure_iterations: 30,
            timeout: Duration::from_secs(30),
        }
    }
}

/// Result of a benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchResult {
    pub name: String,
    pub adapter: String,
    pub iterations: usize,
    pub timings_ms: Vec<f64>,
    pub mean_ms: f64,
    pub median_ms: f64,
    pub stddev_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub cv: f64, // Coefficient of variation
}

/// Run a benchmark function with warmup and measurement.
pub fn run_bench<F>(name: &str, adapter: &str, config: &BenchConfig, mut f: F) -> BenchResult
where
    F: FnMut() -> Duration,
{
    // Warmup
    for _ in 0..config.warmup_iterations {
        f();
    }

    // Measure
    let mut timings_ms: Vec<f64> = Vec::with_capacity(config.measure_iterations);
    for _ in 0..config.measure_iterations {
        let dur = f();
        timings_ms.push(dur.as_secs_f64() * 1000.0);
    }

    timings_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mean_ms: f64 = timings_ms.iter().sum::<f64>() / timings_ms.len() as f64;
    let median_ms = percentile(&timings_ms, 50.0);
    let p95_ms = percentile(&timings_ms, 95.0);
    let p99_ms = percentile(&timings_ms, 99.0);
    let min_ms = timings_ms.first().copied().unwrap_or(0.0);
    let max_ms = timings_ms.last().copied().unwrap_or(0.0);

    let variance: f64 = timings_ms
        .iter()
        .map(|t| (t - mean_ms).powi(2))
        .sum::<f64>()
        / (timings_ms.len() - 1).max(1) as f64;
    let stddev_ms = variance.sqrt();
    let cv = if mean_ms > 0.0 {
        stddev_ms / mean_ms
    } else {
        0.0
    };

    BenchResult {
        name: name.to_string(),
        adapter: adapter.to_string(),
        iterations: config.measure_iterations,
        timings_ms,
        mean_ms,
        median_ms,
        stddev_ms,
        p95_ms,
        p99_ms,
        min_ms,
        max_ms,
        cv,
    }
}

fn percentile(sorted: &[f64], pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((pct / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Format a comparison between two results as markdown.
pub fn compare_markdown(baseline: &BenchResult, contender: &BenchResult) -> String {
    let speedup = baseline.median_ms / contender.median_ms;
    let faster = if speedup > 1.0 { "faster" } else { "slower" };

    format!(
        "| {} | {:.1}ms | {:.1}ms | **{:.2}x** {} |",
        baseline.name, contender.median_ms, baseline.median_ms, speedup, faster,
    )
}
