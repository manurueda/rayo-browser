//! Test result types — collected during execution.

use serde::Serialize;

/// Result of an entire test suite run.
#[derive(Debug, Clone, Serialize)]
pub struct SuiteResult {
    pub name: String,
    pub pass: bool,
    pub total_steps: usize,
    pub passed_steps: usize,
    pub failed_steps: usize,
    pub duration_ms: u64,
    pub steps: Vec<StepResult>,
    pub started_at: String,
}

/// Result of a single test step.
#[derive(Debug, Clone, Serialize)]
pub struct StepResult {
    pub name: String,
    pub pass: bool,
    pub duration_ms: u64,
    /// Action that was executed.
    pub action: String,
    /// Error message if step failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Assertion results (if step had assertions).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assertions: Vec<AssertionResult>,
    /// Page map snapshot at time of assertion (for debugging).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_map: Option<serde_json::Value>,
}

/// Result of a single assertion.
#[derive(Debug, Clone, Serialize)]
pub struct AssertionResult {
    pub assertion_type: String,
    pub pass: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// For screenshot assertions: diff details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_report: Option<serde_json::Value>,
    /// Whether a new baseline was created.
    #[serde(default)]
    pub new_baseline: bool,
}
