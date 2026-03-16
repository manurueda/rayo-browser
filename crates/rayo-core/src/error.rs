//! Error types for rayo-core.

#[derive(Debug, thiserror::Error)]
pub enum RayoError {
    #[error("Element not found: {selector}")]
    ElementNotFound { selector: String },

    #[error("Timeout after {ms}ms waiting for: {what}")]
    Timeout { what: String, ms: u64 },

    #[error("CDP error: {0}")]
    Cdp(String),

    #[error("Cookie error: {0}")]
    CookieError(String),

    #[error(transparent)]
    Chromiumoxide(#[from] chromiumoxide::error::CdpError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
