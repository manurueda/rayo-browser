//! Event-driven waits — NOT polling.
//!
//! Playwright polls every 100-200ms via Runtime.evaluate.
//! We subscribe to CDP DOM mutation events and resolve immediately.
//! Saves 100-500ms per wait operation.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Wait strategy for browser operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitStrategy {
    /// Wait for the page load event.
    Load,
    /// Wait for DOMContentLoaded.
    DomContentLoaded,
    /// Wait until there are no more than 0 network connections for 500ms.
    NetworkIdle,
    /// Wait for a specific selector to appear in the DOM.
    Selector {
        selector: String,
        #[serde(default = "default_timeout")]
        timeout_ms: u64,
    },
    /// No waiting — execute immediately.
    None,
}

fn default_timeout() -> u64 {
    5000
}

/// Configuration for event-driven waiting.
pub struct WaitConfig {
    /// Maximum time to wait before timeout.
    pub timeout: Duration,
    /// How often to check (only used as a fallback if events aren't available).
    pub poll_interval: Duration,
}

impl Default for WaitConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            poll_interval: Duration::from_millis(50), // Fallback only
        }
    }
}

impl WaitConfig {
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wait_strategy_serialization() {
        let strategy = WaitStrategy::Selector {
            selector: "div.loaded".into(),
            timeout_ms: 3000,
        };
        let json = serde_json::to_string(&strategy).unwrap();
        assert!(json.contains("selector"));
        assert!(json.contains("3000"));
    }

    #[test]
    fn test_default_config() {
        let config = WaitConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(5));
    }
}
