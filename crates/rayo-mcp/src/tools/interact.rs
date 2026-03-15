//! rayo_interact tool: click, type, select, scroll.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct InteractInput {
    pub action: InteractAction,
    /// Element ID from page_map.
    #[serde(default)]
    pub id: Option<usize>,
    /// CSS selector (alternative to id).
    #[serde(default)]
    pub selector: Option<String>,
    /// Value for type/select actions.
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractAction {
    Click,
    Type,
    Select,
    Scroll,
    Clear,
    Focus,
}

#[derive(Debug, Serialize)]
pub struct InteractOutput {
    pub success: bool,
    pub duration_ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
