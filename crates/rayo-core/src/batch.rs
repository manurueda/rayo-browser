//! Multi-action batch execution engine.
//!
//! Executes multiple browser actions in a single MCP call.
//! This is the biggest speed win for AI agents:
//! 7 actions in 1 call vs 7 separate MCP round-trips.

use serde::{Deserialize, Serialize};

/// A single action in a batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BatchAction {
    /// Click an element by page map ID or selector.
    Click {
        #[serde(flatten)]
        target: ActionTarget,
    },
    /// Type text into an input.
    Type {
        #[serde(flatten)]
        target: ActionTarget,
        value: String,
    },
    /// Select an option from a dropdown.
    Select {
        #[serde(flatten)]
        target: ActionTarget,
        value: String,
    },
    /// Press a key on an element or the document.
    Press {
        #[serde(flatten)]
        target: Option<ActionTarget>,
        key: String,
    },
    /// Navigate to a URL.
    Goto { url: String },
    /// Take a screenshot.
    Screenshot {
        #[serde(default)]
        full_page: bool,
    },
    /// Wait for an element to appear.
    WaitFor {
        #[serde(flatten)]
        target: ActionTarget,
        #[serde(default = "default_timeout")]
        timeout_ms: u64,
        /// If true, also check that the element is visible (not just present in DOM).
        #[serde(default)]
        visible: Option<bool>,
    },
    /// Scroll to element or position.
    Scroll {
        #[serde(flatten)]
        target: Option<ActionTarget>,
        #[serde(default)]
        x: i32,
        #[serde(default)]
        y: i32,
    },
    /// Hover over an element (mouse move without click).
    Hover {
        #[serde(flatten)]
        target: ActionTarget,
    },
}

fn default_timeout() -> u64 {
    5000
}

/// Target an element by page map ID or CSS selector.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ActionTarget {
    /// Reference by page map element ID.
    Id { id: usize },
    /// Reference by CSS selector.
    Selector { selector: String },
}

/// Result of a single action in a batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchActionResult {
    pub index: usize,
    pub action: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    pub duration_ms: f64,
}

/// Result of an entire batch execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    pub results: Vec<BatchActionResult>,
    pub total_duration_ms: f64,
    pub succeeded: usize,
    pub failed: usize,
}

impl BatchResult {
    pub fn all_succeeded(&self) -> bool {
        self.failed == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_action_deserialization() {
        let json = r#"[
            {"action": "goto", "url": "https://example.com"},
            {"action": "click", "id": 3},
            {"action": "type", "selector": "input[name='q']", "value": "hello"},
            {"action": "screenshot", "full_page": true}
        ]"#;

        let actions: Vec<BatchAction> = serde_json::from_str(json).unwrap();
        assert_eq!(actions.len(), 4);
    }

    #[test]
    fn test_batch_result() {
        let result = BatchResult {
            results: vec![
                BatchActionResult {
                    index: 0,
                    action: "goto".into(),
                    success: true,
                    error: None,
                    data: None,
                    duration_ms: 150.0,
                },
                BatchActionResult {
                    index: 1,
                    action: "click".into(),
                    success: false,
                    error: Some("Element not found".into()),
                    data: None,
                    duration_ms: 5.0,
                },
            ],
            total_duration_ms: 155.0,
            succeeded: 1,
            failed: 1,
        };

        assert!(!result.all_succeeded());
    }
}
