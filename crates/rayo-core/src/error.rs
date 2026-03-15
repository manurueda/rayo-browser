//! Error types for rayo-core.

#[derive(Debug, thiserror::Error)]
pub enum RayoError {
    #[error("Element not found: {selector}")]
    ElementNotFound { selector: String },

    #[error("Navigation failed: {0}")]
    NavigationFailed(String),

    #[error("Timeout after {ms}ms waiting for: {what}")]
    Timeout { what: String, ms: u64 },

    #[error("Batch action {index} failed: {message}")]
    BatchActionFailed { index: usize, message: String },

    #[error("CDP error: {0}")]
    Cdp(String),

    #[error("Cookie error: {0}")]
    CookieError(String),

    #[error("Page not available")]
    PageNotAvailable,

    #[error(transparent)]
    Chromiumoxide(#[from] chromiumoxide::error::CdpError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
