//! Runtime rule evaluation engine.
//!
//! Checks every operation pre/post execution.
//! Must be near-zero-cost: simple string matches and numeric comparisons.

use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::config::{RayoRulesConfig, Severity};

/// A rule violation detected during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    pub rule: String,
    pub severity: Severity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

/// Runtime rule evaluation engine.
pub struct RuleEngine {
    config: RayoRulesConfig,
    violations: Vec<Violation>,
    screenshot_timestamps: Vec<Instant>,
    /// Number of sequential interact/navigate calls without a batch.
    sequential_action_count: usize,
}

impl RuleEngine {
    pub fn new(config: RayoRulesConfig) -> Self {
        Self {
            config,
            violations: Vec::new(),
            screenshot_timestamps: Vec::new(),
            sequential_action_count: 0,
        }
    }

    /// Check a selector before use. Returns violation if slow pattern detected.
    pub fn check_selector(&mut self, selector: &str) -> Option<Violation> {
        // XPath detection
        if selector.starts_with('/') || selector.starts_with("//") {
            let severity = self
                .config
                .rules
                .get("selectors/prefer-css")
                .map(|r| r.severity())
                .unwrap_or(Severity::Off);

            if severity != Severity::Off {
                let v = Violation {
                    rule: "selectors/prefer-css".into(),
                    severity,
                    message: "XPath selectors are 2-10x slower than CSS in CDP.".into(),
                    suggestion: Some(format!("Convert to CSS selector. XPath: {selector}")),
                };
                self.violations.push(v.clone());
                return Some(v);
            }
        }

        // Broad selector detection
        let broad = ["*", "div", "span", "p", "a", "li", "tr", "td"];
        if broad.contains(&selector.trim()) {
            let severity = self
                .config
                .rules
                .get("selectors/no-broad-selectors")
                .map(|r| r.severity())
                .unwrap_or(Severity::Off);

            if severity != Severity::Off {
                let v = Violation {
                    rule: "selectors/no-broad-selectors".into(),
                    severity,
                    message: format!(
                        "Selector \"{selector}\" is too broad. Use a class, ID, or data attribute."
                    ),
                    suggestion: Some("Use a more specific selector like .classname or #id".into()),
                };
                self.violations.push(v.clone());
                return Some(v);
            }
        }

        None
    }

    /// Check if a screenshot is allowed (rate limiting).
    pub fn check_screenshot(&mut self) -> Option<Violation> {
        let now = Instant::now();

        // Clean old timestamps (older than 60s)
        self.screenshot_timestamps
            .retain(|t| now.duration_since(*t).as_secs() < 60);

        let max_per_minute = self
            .config
            .rules
            .get("screenshots/rate-limit")
            .and_then(|r| r.option("maxPerMinute"))
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        if self.screenshot_timestamps.len() >= max_per_minute {
            let severity = self
                .config
                .rules
                .get("screenshots/rate-limit")
                .map(|r| r.severity())
                .unwrap_or(Severity::Off);

            if severity != Severity::Off {
                let v = Violation {
                    rule: "screenshots/rate-limit".into(),
                    severity,
                    message: format!(
                        "Screenshot rate limit exceeded ({max_per_minute}/min). Use rayo_observe with page_map instead."
                    ),
                    suggestion: Some(
                        "Use page_map for page understanding — 200x more token-efficient.".into(),
                    ),
                };
                self.violations.push(v.clone());
                return Some(v);
            }
        }

        self.screenshot_timestamps.push(now);
        None
    }

    /// Check if an operation exceeded its timing budget.
    pub fn check_budget(&mut self, operation: &str, duration_ms: f64) -> Option<Violation> {
        let budget = match operation {
            "cdp_command" => &self.config.budgets.cdp_command,
            "navigation" => &self.config.budgets.navigation,
            "screenshot" => &self.config.budgets.screenshot,
            "dom_read" => &self.config.budgets.dom_read,
            _ => return None,
        };

        if duration_ms > budget.max_ms as f64 && budget.severity != Severity::Off {
            let v = Violation {
                rule: format!("budgets/{operation}"),
                severity: budget.severity,
                message: format!(
                    "{operation} took {duration_ms:.1}ms (budget: {}ms)",
                    budget.max_ms
                ),
                suggestion: None,
            };
            self.violations.push(v.clone());
            return Some(v);
        }

        None
    }

    /// Check if sequential non-batch actions should be combined.
    /// Returns a violation when 3+ sequential actions are detected.
    pub fn check_batch_opportunity(&mut self) -> Option<Violation> {
        self.sequential_action_count += 1;
        if self.sequential_action_count >= 3 {
            let severity = self
                .config
                .rules
                .get("batching/combine-sequential")
                .map(|r| r.severity())
                .unwrap_or(Severity::Off);

            if severity != Severity::Off {
                let v = Violation {
                    rule: "batching/combine-sequential".into(),
                    severity,
                    message: format!(
                        "{} sequential actions detected. Use rayo_batch to combine them into a single call.",
                        self.sequential_action_count
                    ),
                    suggestion: Some(
                        "Use rayo_batch with multiple actions instead of individual rayo_interact calls."
                            .into(),
                    ),
                };
                self.violations.push(v.clone());
                return Some(v);
            }
        }
        None
    }

    /// Reset the sequential action counter (called when batch is used).
    pub fn reset_sequential_count(&mut self) {
        self.sequential_action_count = 0;
    }

    /// Check if page_map should be preferred over screenshot.
    pub fn check_page_map_preference(&mut self) -> Option<Violation> {
        let severity = self
            .config
            .rules
            .get("screenshots/prefer-page-map")
            .map(|r| r.severity())
            .unwrap_or(Severity::Off);

        if severity == Severity::Off {
            return None;
        }

        let v = Violation {
            rule: "screenshots/prefer-page-map".into(),
            severity,
            message: "Consider using page_map instead of screenshot. Page maps are 200x more token-efficient.".into(),
            suggestion: Some("Use rayo_observe with mode 'page_map' instead of 'screenshot'.".into()),
        };
        self.violations.push(v.clone());
        Some(v)
    }

    /// Get all accumulated violations.
    pub fn violations(&self) -> &[Violation] {
        &self.violations
    }

    /// Get recent violations (since last drain).
    pub fn drain_violations(&mut self) -> Vec<Violation> {
        std::mem::take(&mut self.violations)
    }

    /// Get screenshot rate limit info: (screenshots_remaining, reset_in_ms).
    /// Returns the number of screenshots still allowed in the current window and
    /// milliseconds until the oldest timestamp expires (resetting a slot).
    pub fn screenshot_rate_info(&self) -> (usize, u64) {
        let now = Instant::now();

        let max_per_minute = self
            .config
            .rules
            .get("screenshots/rate-limit")
            .and_then(|r| r.option("maxPerMinute"))
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        // Count only timestamps within the 60-second window
        let active: Vec<_> = self
            .screenshot_timestamps
            .iter()
            .filter(|t| now.duration_since(**t).as_secs() < 60)
            .collect();

        let remaining = max_per_minute.saturating_sub(active.len());

        let reset_in_ms = if let Some(oldest) = active.first() {
            let elapsed = now.duration_since(**oldest).as_millis() as u64;
            60_000u64.saturating_sub(elapsed)
        } else {
            0
        };

        (remaining, reset_in_ms)
    }

    /// Get the rules config.
    pub fn config(&self) -> &RayoRulesConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xpath_detection() {
        let config = RayoRulesConfig::default();
        let mut engine = RuleEngine::new(config);

        let v = engine.check_selector("//div[@class='foo']");
        assert!(v.is_some());
        assert_eq!(v.unwrap().rule, "selectors/prefer-css");
    }

    #[test]
    fn test_css_ok() {
        let config = RayoRulesConfig::default();
        let mut engine = RuleEngine::new(config);

        let v = engine.check_selector("div.foo");
        assert!(v.is_none());
    }

    #[test]
    fn test_broad_selector() {
        let config = RayoRulesConfig::default();
        let mut engine = RuleEngine::new(config);

        let v = engine.check_selector("div");
        assert!(v.is_some());
        assert_eq!(v.unwrap().rule, "selectors/no-broad-selectors");
    }

    #[test]
    fn test_budget_exceeded() {
        let config = RayoRulesConfig::default();
        let mut engine = RuleEngine::new(config);

        // CDP command budget is 50ms
        let v = engine.check_budget("cdp_command", 100.0);
        assert!(v.is_some());

        // Under budget
        let v = engine.check_budget("cdp_command", 30.0);
        assert!(v.is_none());
    }

    #[test]
    fn test_batch_opportunity_warning() {
        let config = RayoRulesConfig::default();
        let mut engine = RuleEngine::new(config);

        // First two calls: no violation
        assert!(engine.check_batch_opportunity().is_none());
        assert!(engine.check_batch_opportunity().is_none());

        // Third call: violation triggered
        let v = engine.check_batch_opportunity();
        assert!(v.is_some());
        let v = v.unwrap();
        assert_eq!(v.rule, "batching/combine-sequential");
        assert!(v.message.contains("3 sequential actions"));
    }

    #[test]
    fn test_batch_opportunity_reset() {
        let config = RayoRulesConfig::default();
        let mut engine = RuleEngine::new(config);

        // Two calls, then reset
        assert!(engine.check_batch_opportunity().is_none());
        assert!(engine.check_batch_opportunity().is_none());
        engine.reset_sequential_count();

        // Two more calls — still no violation (counter was reset)
        assert!(engine.check_batch_opportunity().is_none());
        assert!(engine.check_batch_opportunity().is_none());
    }

    #[test]
    fn test_page_map_preference() {
        let config = RayoRulesConfig::default();
        let mut engine = RuleEngine::new(config);

        let v = engine.check_page_map_preference();
        assert!(v.is_some());
        let v = v.unwrap();
        assert_eq!(v.rule, "screenshots/prefer-page-map");
        assert!(v.message.contains("page_map"));
        assert!(v.suggestion.unwrap().contains("page_map"));
    }
}
