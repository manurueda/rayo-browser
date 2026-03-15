//! rayo_navigate tool: goto, reload, back, forward.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct NavigateInput {
    pub action: NavigateAction,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default = "default_wait_until")]
    pub wait_until: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NavigateAction {
    Goto,
    Reload,
    Back,
    Forward,
}

fn default_wait_until() -> String {
    "load".into()
}

#[derive(Debug, Serialize)]
pub struct NavigateOutput {
    pub url: String,
    pub title: String,
    pub duration_ms: f64,
}
