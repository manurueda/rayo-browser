//! Collects tool errors for bug reporting via `rayo_report`.
//!
//! Ring buffer (max 50 entries). Version injected at construction for testability.

use std::collections::VecDeque;

const MAX_ERRORS: usize = 50;

/// A recorded tool error with full context for issue filing.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ErrorRecord {
    pub timestamp: String,
    pub tool: String,
    pub params: serde_json::Map<String, serde_json::Value>,
    pub error: String,
    pub version: String,
}

/// Collects recent tool errors (ring buffer, max 50).
pub struct ErrorCollector {
    errors: VecDeque<ErrorRecord>,
    version: String,
}

impl ErrorCollector {
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            errors: VecDeque::with_capacity(MAX_ERRORS),
            version: version.into(),
        }
    }

    pub fn record(
        &mut self,
        tool: String,
        params: serde_json::Map<String, serde_json::Value>,
        error: String,
    ) {
        self.record_with_timestamp(tool, params, error, chrono::Utc::now().to_rfc3339());
    }

    /// Record with an explicit timestamp (for deterministic testing).
    pub fn record_with_timestamp(
        &mut self,
        tool: String,
        params: serde_json::Map<String, serde_json::Value>,
        error: String,
        timestamp: String,
    ) {
        if self.errors.len() >= MAX_ERRORS {
            self.errors.pop_front();
        }
        self.errors.push_back(ErrorRecord {
            timestamp,
            tool,
            params,
            error,
            version: self.version.clone(),
        });
    }

    pub fn report(&self) -> &VecDeque<ErrorRecord> {
        &self.errors
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn clear(&mut self) {
        self.errors.clear();
    }

    pub fn len(&self) -> usize {
        self.errors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_params(key: &str, val: &str) -> serde_json::Map<String, serde_json::Value> {
        let mut m = serde_json::Map::new();
        m.insert(key.to_string(), serde_json::Value::String(val.to_string()));
        m
    }

    #[test]
    fn record_and_report() {
        let mut c = ErrorCollector::new("0.1.0-test");
        assert!(c.is_empty());

        c.record_with_timestamp(
            "rayo_navigate".into(),
            make_params("url", "https://example.com"),
            "timeout".into(),
            "2026-01-01T00:00:00Z".into(),
        );

        assert_eq!(c.len(), 1);
        let errors = c.report();
        assert_eq!(errors[0].tool, "rayo_navigate");
        assert_eq!(errors[0].error, "timeout");
        assert_eq!(errors[0].version, "0.1.0-test");
        assert_eq!(errors[0].timestamp, "2026-01-01T00:00:00Z");
    }

    #[test]
    fn ring_buffer_eviction() {
        let mut c = ErrorCollector::new("0.1.0");

        // Fill to capacity + 5
        for i in 0..55 {
            c.record_with_timestamp(
                format!("tool_{i}"),
                serde_json::Map::new(),
                format!("error_{i}"),
                format!("ts_{i}"),
            );
        }

        assert_eq!(c.len(), 50);
        // Oldest 5 should be evicted — first remaining should be tool_5
        assert_eq!(c.report()[0].tool, "tool_5");
        // Newest should be tool_54
        assert_eq!(c.report().back().unwrap().tool, "tool_54");
    }

    #[test]
    fn clear_resets() {
        let mut c = ErrorCollector::new("0.1.0");
        c.record_with_timestamp("t".into(), serde_json::Map::new(), "e".into(), "ts".into());
        assert!(!c.is_empty());

        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn version_injection() {
        let c = ErrorCollector::new("custom-version");
        assert_eq!(c.version(), "custom-version");
    }

    #[test]
    fn serializes_to_json() {
        let mut c = ErrorCollector::new("0.2.0");
        c.record_with_timestamp(
            "rayo_observe".into(),
            make_params("mode", "page_map"),
            "CDP error: session closed".into(),
            "2026-03-22T12:00:00Z".into(),
        );

        let json = serde_json::to_string(&c.report()[0]).unwrap();
        assert!(json.contains("rayo_observe"));
        assert!(json.contains("CDP error"));
        assert!(json.contains("0.2.0"));
    }
}
