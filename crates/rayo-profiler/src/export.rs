//! Export profiling data to various formats.

use crate::ProfileSpan;
use crate::stats::CategoryStats;
use serde::Serialize;

/// Export spans to JSON string.
pub fn to_json(spans: &[ProfileSpan]) -> String {
    serde_json::to_string_pretty(spans).unwrap_or_default()
}

/// Chrome DevTools Trace Event Format.
/// Load in chrome://tracing or Perfetto UI.
#[derive(Serialize)]
struct TraceEvent {
    pid: u32,
    tid: u32,
    ts: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    dur: Option<u64>,
    ph: &'static str,
    name: String,
    cat: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    args: serde_json::Value,
}

#[derive(Serialize)]
struct TraceOutput {
    #[serde(rename = "traceEvents")]
    trace_events: Vec<TraceEvent>,
}

pub fn to_chrome_trace(spans: &[ProfileSpan]) -> String {
    let events: Vec<TraceEvent> = spans
        .iter()
        .map(|span| TraceEvent {
            pid: 1,
            tid: span.category as u32,
            ts: span.start_us,
            dur: span.duration_us,
            ph: "X", // Complete event
            name: span.name.clone(),
            cat: span.category.as_str().to_string(),
            args: span.metadata.clone(),
        })
        .collect();

    let output = TraceOutput {
        trace_events: events,
    };
    serde_json::to_string(&output).unwrap_or_default()
}

/// Markdown table of category statistics.
pub fn to_markdown(stats: &[CategoryStats], spans: &[ProfileSpan]) -> String {
    let total_us: u64 = stats.iter().map(|s| s.total_us).max().unwrap_or(0);
    let total_ms = total_us as f64 / 1000.0;

    let mut out = format!("## Rayo Profile ({total_ms:.1}ms total)\n\n");
    out.push_str("| Category | Total | Count | Avg | p95 | % |\n");
    out.push_str("|----------|-------|-------|-----|-----|---|\n");

    for s in stats {
        out.push_str(&format!(
            "| {} | {:.1}ms | {} | {:.1}ms | {:.1}ms | {:.1}% |\n",
            s.category.as_str(),
            s.total_us as f64 / 1000.0,
            s.count,
            s.avg_us as f64 / 1000.0,
            s.p95_us as f64 / 1000.0,
            s.percent_of_total,
        ));
    }

    // Top 5 slowest operations
    let mut sorted: Vec<_> = spans.iter().filter(|s| s.duration_us.is_some()).collect();
    sorted.sort_by(|a, b| b.duration_us.cmp(&a.duration_us));

    if !sorted.is_empty() {
        out.push_str("\n### Slowest Operations\n\n");
        out.push_str("| Operation | Duration |\n");
        out.push_str("|-----------|----------|\n");
        for span in sorted.iter().take(5) {
            out.push_str(&format!(
                "| {} | {:.1}ms |\n",
                span.name,
                span.duration_us.unwrap() as f64 / 1000.0,
            ));
        }
    }

    out
}

/// Token-efficient AI agent summary.
/// Designed for LLM consumption — minimal tokens, maximum signal.
pub fn to_ai_summary(stats: &[CategoryStats], spans: &[ProfileSpan]) -> String {
    let total_us: u64 = spans
        .iter()
        .filter(|s| s.parent_id.is_none())
        .filter_map(|s| s.duration_us)
        .sum();
    let total_ms = total_us as f64 / 1000.0;

    let mut out = format!("RAYO PROFILE ({total_ms:.1}ms total)\n");

    for s in stats {
        out.push_str(&format!(
            "  {}: {:.1}ms ({:.1}%) | {} ops | avg {:.1}ms | p95 {:.1}ms\n",
            s.category.as_str(),
            s.total_us as f64 / 1000.0,
            s.percent_of_total,
            s.count,
            s.avg_us as f64 / 1000.0,
            s.p95_us as f64 / 1000.0,
        ));
    }

    // Top 3 slowest
    let mut sorted: Vec<_> = spans.iter().filter(|s| s.duration_us.is_some()).collect();
    sorted.sort_by(|a, b| b.duration_us.cmp(&a.duration_us));

    if !sorted.is_empty() {
        out.push_str("SLOWEST:");
        for span in sorted.iter().take(3) {
            out.push_str(&format!(
                " {}({:.1}ms)",
                span.name,
                span.duration_us.unwrap() as f64 / 1000.0,
            ));
        }
        out.push('\n');
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SpanCategory;

    #[test]
    fn test_chrome_trace_format() {
        let spans = vec![ProfileSpan {
            id: 1,
            parent_id: None,
            name: "test".into(),
            category: SpanCategory::Navigation,
            start_us: 1000,
            duration_us: Some(5000),
            metadata: serde_json::Value::Null,
        }];

        let trace = to_chrome_trace(&spans);
        let parsed: serde_json::Value = serde_json::from_str(&trace).unwrap();
        assert!(parsed["traceEvents"].is_array());
        assert_eq!(parsed["traceEvents"][0]["name"], "test");
        assert_eq!(parsed["traceEvents"][0]["ph"], "X");
    }

    #[test]
    fn test_ai_summary_format() {
        let spans = vec![ProfileSpan {
            id: 1,
            parent_id: None,
            name: "goto".into(),
            category: SpanCategory::Navigation,
            start_us: 0,
            duration_us: Some(100_000),
            metadata: serde_json::Value::Null,
        }];

        let stats = crate::stats::compute_category_stats(&spans);
        let summary = to_ai_summary(&stats, &spans);
        assert!(summary.starts_with("RAYO PROFILE"));
        assert!(summary.contains("navigation"));
    }
}
