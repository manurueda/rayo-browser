//! rayo_profile tool: get profiling results.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ProfileInput {
    #[serde(default = "default_format")]
    pub format: ProfileFormat,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileFormat {
    /// AI-readable summary (default, most token-efficient).
    AiSummary,
    /// Full JSON with all spans.
    Json,
    /// Markdown table.
    Markdown,
    /// Chrome DevTools trace format.
    ChromeTrace,
}

fn default_format() -> ProfileFormat {
    ProfileFormat::AiSummary
}

#[derive(Debug, Serialize)]
pub struct ProfileOutput {
    pub format: String,
    pub data: String,
    pub span_count: usize,
}
