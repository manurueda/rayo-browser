use thiserror::Error;

#[derive(Debug, Error)]
pub enum VisualError {
    #[error(
        "dimension mismatch: baseline {baseline_w}x{baseline_h} vs current {current_w}x{current_h}"
    )]
    DimensionMismatch {
        baseline_w: u32,
        baseline_h: u32,
        current_w: u32,
        current_h: u32,
    },

    #[error("invalid baseline name '{name}': only [a-zA-Z0-9_-] allowed, no path separators")]
    InvalidName { name: String },

    #[error("baseline not found: '{name}'")]
    BaselineNotFound { name: String },

    #[error("image decode error: {0}")]
    ImageDecode(#[from] image::ImageError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
