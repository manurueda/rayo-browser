//! rayo_network tool: capture, block, mock network requests.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct NetworkInput {
    pub mode: NetworkMode,
    /// URL pattern for block/mock (glob-style: * matches anything).
    #[serde(default)]
    pub url_pattern: Option<String>,
    /// Resource type filter (script, stylesheet, image, document, etc.).
    #[serde(default)]
    pub resource_type: Option<String>,
    /// Mock response configuration (for mock mode).
    #[serde(default)]
    pub response: Option<MockResponse>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkMode {
    /// Start capturing network requests.
    Capture,
    /// Get captured requests (optionally filtered by url_pattern).
    Requests,
    /// Block requests matching url_pattern.
    Block,
    /// Mock responses for requests matching url_pattern.
    Mock,
    /// Clear all rules and captured requests.
    Clear,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MockResponse {
    #[serde(default = "default_status")]
    pub status: u16,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
}

fn default_status() -> u16 {
    200
}
