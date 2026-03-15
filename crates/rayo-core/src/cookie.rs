//! Cookie types owned by rayo-core.
//!
//! These decouple downstream crates from chromiumoxide's internal CDP types.

use serde::{Deserialize, Serialize};

/// Input for setting a cookie. Converted to CDP CookieParam internally.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCookie {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secure: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub same_site: Option<SameSite>,
    /// Expiration as seconds since Unix epoch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<f64>,
}

/// SameSite cookie attribute.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SameSite {
    Strict,
    Lax,
    None,
}

/// Cookie returned from the browser. Read-only view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieInfo {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub same_site: Option<String>,
    pub expires: f64,
}
