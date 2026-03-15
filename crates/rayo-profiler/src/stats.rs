//! Statistical analysis for profiling data.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{ProfileSpan, SpanCategory};

/// Aggregate statistics for a span category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryStats {
    pub category: SpanCategory,
    pub count: usize,
    pub total_us: u64,
    pub avg_us: u64,
    pub min_us: u64,
    pub max_us: u64,
    pub median_us: u64,
    pub p95_us: u64,
    pub p99_us: u64,
    pub stddev_us: f64,
    /// Percentage of total profiled time.
    pub percent_of_total: f64,
}

/// Compute per-category statistics from a list of spans.
pub fn compute_category_stats(spans: &[ProfileSpan]) -> Vec<CategoryStats> {
    let total_duration: u64 = spans
        .iter()
        .filter(|s| s.parent_id.is_none()) // Only root spans for total
        .filter_map(|s| s.duration_us)
        .sum();

    let mut by_category: HashMap<SpanCategory, Vec<u64>> = HashMap::new();

    for span in spans {
        if let Some(dur) = span.duration_us {
            by_category.entry(span.category).or_default().push(dur);
        }
    }

    let mut stats: Vec<CategoryStats> = by_category
        .into_iter()
        .map(|(category, mut durations)| {
            durations.sort_unstable();
            let count = durations.len();
            let total_us: u64 = durations.iter().sum();
            let avg_us = if count > 0 { total_us / count as u64 } else { 0 };
            let min_us = durations.first().copied().unwrap_or(0);
            let max_us = durations.last().copied().unwrap_or(0);
            let median_us = percentile(&durations, 50.0);
            let p95_us = percentile(&durations, 95.0);
            let p99_us = percentile(&durations, 99.0);
            let stddev_us = stddev(&durations, avg_us);
            let percent_of_total = if total_duration > 0 {
                (total_us as f64 / total_duration as f64) * 100.0
            } else {
                0.0
            };

            CategoryStats {
                category,
                count,
                total_us,
                avg_us,
                min_us,
                max_us,
                median_us,
                p95_us,
                p99_us,
                stddev_us,
                percent_of_total,
            }
        })
        .collect();

    // Sort by total time descending
    stats.sort_by(|a, b| b.total_us.cmp(&a.total_us));
    stats
}

fn percentile(sorted: &[u64], pct: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((pct / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn stddev(values: &[u64], mean: u64) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let variance: f64 = values
        .iter()
        .map(|&v| {
            let diff = v as f64 - mean as f64;
            diff * diff
        })
        .sum::<f64>()
        / (values.len() - 1) as f64;
    variance.sqrt()
}

/// Welch's t-test for comparing two samples.
/// Returns (t_statistic, p_value_approximate, is_significant_at_005).
pub fn welch_t_test(a: &[f64], b: &[f64]) -> (f64, bool) {
    if a.len() < 2 || b.len() < 2 {
        return (0.0, false);
    }

    let mean_a: f64 = a.iter().sum::<f64>() / a.len() as f64;
    let mean_b: f64 = b.iter().sum::<f64>() / b.len() as f64;

    let var_a: f64 = a.iter().map(|x| (x - mean_a).powi(2)).sum::<f64>() / (a.len() - 1) as f64;
    let var_b: f64 = b.iter().map(|x| (x - mean_b).powi(2)).sum::<f64>() / (b.len() - 1) as f64;

    let se = (var_a / a.len() as f64 + var_b / b.len() as f64).sqrt();
    if se == 0.0 {
        return (0.0, false);
    }

    let t = (mean_a - mean_b) / se;

    // Approximate: |t| > 2.0 is roughly p < 0.05 for reasonable sample sizes
    let significant = t.abs() > 2.0;

    (t, significant)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SpanCategory;

    #[test]
    fn test_category_stats() {
        let spans = vec![
            ProfileSpan {
                id: 1,
                parent_id: None,
                name: "nav1".into(),
                category: SpanCategory::Navigation,
                start_us: 0,
                duration_us: Some(100_000),
                metadata: serde_json::Value::Null,
            },
            ProfileSpan {
                id: 2,
                parent_id: None,
                name: "nav2".into(),
                category: SpanCategory::Navigation,
                start_us: 100_000,
                duration_us: Some(200_000),
                metadata: serde_json::Value::Null,
            },
            ProfileSpan {
                id: 3,
                parent_id: None,
                name: "click".into(),
                category: SpanCategory::DomMutate,
                start_us: 300_000,
                duration_us: Some(50_000),
                metadata: serde_json::Value::Null,
            },
        ];

        let stats = compute_category_stats(&spans);
        assert_eq!(stats.len(), 2);
        // Navigation should be first (more total time)
        assert_eq!(stats[0].category, SpanCategory::Navigation);
        assert_eq!(stats[0].count, 2);
        assert_eq!(stats[0].total_us, 300_000);
    }

    #[test]
    fn test_welch_t_test() {
        // Clearly different distributions
        let a = vec![10.0, 11.0, 12.0, 10.5, 11.5];
        let b = vec![20.0, 21.0, 22.0, 20.5, 21.5];
        let (t, significant) = welch_t_test(&a, &b);
        assert!(t < 0.0); // a < b
        assert!(significant);

        // Similar distributions
        let c = vec![10.0, 10.1, 10.2, 9.9, 10.0];
        let d = vec![10.1, 10.0, 10.2, 9.8, 10.1];
        let (_, significant2) = welch_t_test(&c, &d);
        assert!(!significant2);
    }
}
