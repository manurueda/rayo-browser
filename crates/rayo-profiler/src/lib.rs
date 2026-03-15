//! rayo-profiler: Built-in profiling for rayo-browser
//!
//! Instruments every browser operation with nanosecond-precision timing.
//! Enabled by default. Compile with `--no-default-features` to eliminate all overhead.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐     ┌────────────┐     ┌────────────┐
//! │ profile_span!├────►│ Collector  ├────►│  Exporter  │
//! │  (macro)     │     │ (ring buf) │     │ (json/md)  │
//! └─────────────┘     └────────────┘     └────────────┘
//! ```

pub mod collector;
pub mod export;
pub mod stats;

use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};

/// Span categories for grouping profiling data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanCategory {
    CdpCommand,
    CdpEvent,
    SelectorResolve,
    SelectorCache,
    Navigation,
    Wait,
    Screenshot,
    DomRead,
    DomMutate,
    NetworkIntercept,
    Batch,
    PageMap,
    RuleEvaluation,
    User,
}

impl SpanCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CdpCommand => "cdp.command",
            Self::CdpEvent => "cdp.event",
            Self::SelectorResolve => "selector.resolve",
            Self::SelectorCache => "selector.cache",
            Self::Navigation => "navigation",
            Self::Wait => "wait",
            Self::Screenshot => "screenshot",
            Self::DomRead => "dom.read",
            Self::DomMutate => "dom.mutate",
            Self::NetworkIntercept => "network.intercept",
            Self::Batch => "batch",
            Self::PageMap => "page_map",
            Self::RuleEvaluation => "rule.eval",
            Self::User => "user",
        }
    }
}

/// A single profiling span representing a timed operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSpan {
    pub id: u64,
    pub parent_id: Option<u64>,
    pub name: String,
    pub category: SpanCategory,
    pub start_us: u64,
    pub duration_us: Option<u64>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Active span guard. Ends the span when dropped.
pub struct SpanGuard {
    id: u64,
    start: Instant,
    collector: Arc<Mutex<collector::Collector>>,
}

impl SpanGuard {
    /// End the span and record its duration.
    pub fn end(self) {
        // Drop triggers recording
        drop(self);
    }
}

impl Drop for SpanGuard {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        if let Ok(mut c) = self.collector.lock() {
            c.end_span(self.id, duration);
        }
    }
}

/// The main profiler. Thread-safe, shareable via Arc.
#[derive(Clone)]
pub struct Profiler {
    collector: Arc<Mutex<collector::Collector>>,
    epoch: Instant,
}

impl Profiler {
    /// Create a new profiler with default capacity (100k spans).
    pub fn new() -> Self {
        Self::with_capacity(100_000)
    }

    /// Create a profiler with custom span capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            collector: Arc::new(Mutex::new(collector::Collector::new(capacity))),
            epoch: Instant::now(),
        }
    }

    /// Start a new profiling span. Returns a guard that ends the span on drop.
    pub fn start_span(&self, name: impl Into<String>, category: SpanCategory) -> SpanGuard {
        let start = Instant::now();
        let start_us = start.duration_since(self.epoch).as_micros() as u64;
        let id = {
            let mut c = self.collector.lock().unwrap();
            c.start_span(name.into(), category, start_us, None)
        };
        SpanGuard {
            id,
            start,
            collector: self.collector.clone(),
        }
    }

    /// Start a child span under a parent.
    pub fn start_child_span(
        &self,
        name: impl Into<String>,
        category: SpanCategory,
        parent_id: u64,
    ) -> SpanGuard {
        let start = Instant::now();
        let start_us = start.duration_since(self.epoch).as_micros() as u64;
        let id = {
            let mut c = self.collector.lock().unwrap();
            c.start_span(name.into(), category, start_us, Some(parent_id))
        };
        SpanGuard {
            id,
            start,
            collector: self.collector.clone(),
        }
    }

    /// Get all collected spans.
    pub fn spans(&self) -> Vec<ProfileSpan> {
        self.collector.lock().unwrap().spans()
    }

    /// Get aggregate statistics by category.
    pub fn category_stats(&self) -> Vec<stats::CategoryStats> {
        let spans = self.spans();
        stats::compute_category_stats(&spans)
    }

    /// Export to JSON string.
    pub fn export_json(&self) -> String {
        export::to_json(&self.spans())
    }

    /// Export to Chrome DevTools trace format.
    pub fn export_chrome_trace(&self) -> String {
        export::to_chrome_trace(&self.spans())
    }

    /// Export to markdown table.
    pub fn export_markdown(&self) -> String {
        let spans = self.spans();
        let stats = stats::compute_category_stats(&spans);
        export::to_markdown(&stats, &spans)
    }

    /// Export AI-readable summary (token-efficient).
    pub fn export_ai_summary(&self) -> String {
        let spans = self.spans();
        let stats = stats::compute_category_stats(&spans);
        export::to_ai_summary(&stats, &spans)
    }

    /// Reset all collected spans.
    pub fn reset(&self) {
        self.collector.lock().unwrap().reset();
    }

    /// Total number of recorded spans.
    pub fn span_count(&self) -> usize {
        self.collector.lock().unwrap().span_count()
    }
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience macro to profile a block of code.
///
/// ```rust
/// use rayo_profiler::{Profiler, SpanCategory, profile_span};
///
/// let profiler = Profiler::new();
/// let result = profile_span!(profiler, "my_operation", SpanCategory::User, {
///     42
/// });
/// assert_eq!(result, 42);
/// ```
#[macro_export]
macro_rules! profile_span {
    ($profiler:expr, $name:expr, $cat:expr, $body:expr) => {{
        let _guard = $profiler.start_span($name, $cat);
        let result = $body;
        drop(_guard);
        result
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_span() {
        let profiler = Profiler::new();
        {
            let _span = profiler.start_span("test_op", SpanCategory::User);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let spans = profiler.spans();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].name, "test_op");
        assert!(spans[0].duration_us.unwrap() >= 10_000); // at least 10ms
    }

    #[test]
    fn test_profile_span_macro() {
        let profiler = Profiler::new();
        let result = profile_span!(profiler, "macro_test", SpanCategory::User, { 42 });
        assert_eq!(result, 42);
        assert_eq!(profiler.span_count(), 1);
    }

    #[test]
    fn test_child_span() {
        let profiler = Profiler::new();
        let parent = profiler.start_span("parent", SpanCategory::Batch);
        let parent_id = parent.id;
        {
            let _child = profiler.start_child_span("child", SpanCategory::CdpCommand, parent_id);
        }
        drop(parent);

        let spans = profiler.spans();
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[1].parent_id, Some(spans[0].id));
    }

    #[test]
    fn test_export_ai_summary() {
        let profiler = Profiler::new();
        {
            let _s = profiler.start_span("goto", SpanCategory::Navigation);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        {
            let _s = profiler.start_span("click", SpanCategory::DomMutate);
            std::thread::sleep(std::time::Duration::from_millis(3));
        }
        let summary = profiler.export_ai_summary();
        assert!(summary.contains("RAYO PROFILE"));
        assert!(summary.contains("navigation"));
    }
}
