//! rayo_observe tool: page_map, text, screenshot, accessibility tree.
//!
//! The key AI tool. page_map returns ~500 tokens vs screenshot's ~100k.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ObserveInput {
    pub mode: ObserveMode,
    /// CSS selector to scope observation (optional).
    #[serde(default)]
    pub selector: Option<String>,
    /// For screenshot mode.
    #[serde(default)]
    pub full_page: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObserveMode {
    /// Token-efficient page map (~500 tokens). DEFAULT.
    PageMap,
    /// Raw text content of the page or element.
    Text,
    /// Screenshot as base64 PNG.
    Screenshot,
    /// Accessibility tree.
    Accessibility,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ObserveOutput {
    PageMap(rayo_core::page_map::PageMap),
    Text {
        text: String,
        duration_ms: f64,
    },
    Screenshot {
        base64: String,
        duration_ms: f64,
    },
    Accessibility {
        tree: serde_json::Value,
        duration_ms: f64,
    },
}
