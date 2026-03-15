//! .rayo-rules file parsing.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Root configuration from .rayo-rules file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RayoRulesConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub rules: HashMap<String, RuleConfig>,
    #[serde(default)]
    pub budgets: BudgetConfig,
    #[serde(default)]
    pub agent: AgentConfig,
}

fn default_version() -> u32 {
    1
}

/// Per-rule configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RuleConfig {
    /// Just a severity level.
    Severity(Severity),
    /// Full config with options.
    Full {
        severity: Severity,
        #[serde(default)]
        options: HashMap<String, serde_json::Value>,
    },
}

impl RuleConfig {
    pub fn severity(&self) -> Severity {
        match self {
            Self::Severity(s) => *s,
            Self::Full { severity, .. } => *severity,
        }
    }

    pub fn option(&self, key: &str) -> Option<&serde_json::Value> {
        match self {
            Self::Severity(_) => None,
            Self::Full { options, .. } => options.get(key),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Off,
    Warn,
    Error,
}

/// Timing budgets by operation type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    #[serde(default = "default_cdp_budget")]
    pub cdp_command: BudgetEntry,
    #[serde(default = "default_nav_budget")]
    pub navigation: BudgetEntry,
    #[serde(default = "default_screenshot_budget")]
    pub screenshot: BudgetEntry,
    #[serde(default = "default_dom_read_budget")]
    pub dom_read: BudgetEntry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetEntry {
    pub max_ms: u64,
    #[serde(default = "default_warn")]
    pub severity: Severity,
}

fn default_warn() -> Severity {
    Severity::Warn
}

fn default_cdp_budget() -> BudgetEntry {
    BudgetEntry { max_ms: 50, severity: Severity::Warn }
}
fn default_nav_budget() -> BudgetEntry {
    BudgetEntry { max_ms: 5000, severity: Severity::Warn }
}
fn default_screenshot_budget() -> BudgetEntry {
    BudgetEntry { max_ms: 200, severity: Severity::Warn }
}
fn default_dom_read_budget() -> BudgetEntry {
    BudgetEntry { max_ms: 300, severity: Severity::Warn }
}

/// AI agent-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    #[serde(default = "default_rate_limit")]
    pub screenshot_rate_limit: u32,
    #[serde(default = "default_cooldown")]
    pub dom_read_cooldown_ms: u64,
    #[serde(default = "default_concise")]
    pub guidance_format: String,
}

fn default_rate_limit() -> u32 {
    10
}
fn default_cooldown() -> u64 {
    2000
}
fn default_concise() -> String {
    "concise".into()
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            cdp_command: default_cdp_budget(),
            navigation: default_nav_budget(),
            screenshot: default_screenshot_budget(),
            dom_read: default_dom_read_budget(),
        }
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            screenshot_rate_limit: default_rate_limit(),
            dom_read_cooldown_ms: default_cooldown(),
            guidance_format: default_concise(),
        }
    }
}

impl Default for RayoRulesConfig {
    fn default() -> Self {
        Self {
            version: 1,
            rules: defaults::default_rules(),
            budgets: BudgetConfig::default(),
            agent: AgentConfig::default(),
        }
    }
}

impl RayoRulesConfig {
    /// Load from a .rayo-rules file. Falls back to defaults if not found.
    pub fn load(path: &Path) -> Self {
        if let Ok(content) = std::fs::read_to_string(path) {
            // Strip JSON comments (// style)
            let stripped: String = content
                .lines()
                .map(|line| {
                    if let Some(idx) = line.find("//") {
                        // Don't strip if inside a string
                        let before = &line[..idx];
                        if before.matches('"').count() % 2 == 0 {
                            return before;
                        }
                    }
                    line
                })
                .collect::<Vec<_>>()
                .join("\n");

            serde_json::from_str(&stripped).unwrap_or_default()
        } else {
            Self::default()
        }
    }
}

use crate::defaults;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RayoRulesConfig::default();
        assert_eq!(config.version, 1);
        assert!(!config.rules.is_empty());
        assert_eq!(config.budgets.cdp_command.max_ms, 50);
    }

    #[test]
    fn test_parse_jsonc() {
        let jsonc = r#"{
            "version": 1,
            // This is a comment
            "rules": {
                "selectors/prefer-css": "error"
            }
        }"#;

        let config: RayoRulesConfig = {
            let stripped: String = jsonc
                .lines()
                .map(|line| {
                    if let Some(idx) = line.find("//") {
                        let before = &line[..idx];
                        if before.matches('"').count() % 2 == 0 {
                            return before;
                        }
                    }
                    line
                })
                .collect::<Vec<_>>()
                .join("\n");
            serde_json::from_str(&stripped).unwrap()
        };

        assert_eq!(config.version, 1);
    }
}
