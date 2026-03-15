//! High-level browser actions.
//!
//! These wrap chromiumoxide's low-level CDP operations with
//! selector caching, profiling, and batch execution support.

use serde::{Deserialize, Serialize};

/// Result of a browser action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub duration_ms: f64,
}

/// Navigation options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigateOptions {
    pub url: String,
    #[serde(default = "default_wait_until")]
    pub wait_until: String,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_wait_until() -> String {
    "load".into()
}

fn default_timeout() -> u64 {
    10_000
}

/// Click options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickOptions {
    /// Element page map ID or CSS selector.
    #[serde(default)]
    pub id: Option<usize>,
    #[serde(default)]
    pub selector: Option<String>,
    /// Number of clicks (1 for single, 2 for double).
    #[serde(default = "default_click_count")]
    pub click_count: u32,
}

fn default_click_count() -> u32 {
    1
}

/// Type text options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeOptions {
    #[serde(default)]
    pub id: Option<usize>,
    #[serde(default)]
    pub selector: Option<String>,
    pub text: String,
    /// Clear existing text before typing.
    #[serde(default = "default_true")]
    pub clear: bool,
}

fn default_true() -> bool {
    true
}

/// Screenshot options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotOptions {
    #[serde(default)]
    pub full_page: bool,
    #[serde(default = "default_format")]
    pub format: String,
    /// Quality (1-100, JPEG only).
    #[serde(default)]
    pub quality: Option<u32>,
}

fn default_format() -> String {
    "png".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigate_options_defaults() {
        let json = r#"{"url": "https://example.com"}"#;
        let opts: NavigateOptions = serde_json::from_str(json).unwrap();
        assert_eq!(opts.wait_until, "load");
        assert_eq!(opts.timeout_ms, 10_000);
    }

    #[test]
    fn test_click_by_id() {
        let json = r#"{"id": 3}"#;
        let opts: ClickOptions = serde_json::from_str(json).unwrap();
        assert_eq!(opts.id, Some(3));
    }
}
