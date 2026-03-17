use thiserror::Error;

#[derive(Debug, Error)]
pub enum TestError {
    #[error("YAML parse error in {path}: {source}")]
    YamlParse {
        path: String,
        source: serde_yaml::Error,
    },

    #[error("no test files found in {path}")]
    NoTestFiles { path: String },

    #[error("browser error: {0}")]
    Browser(#[from] rayo_core::RayoError),

    #[error("visual diff error: {0}")]
    Visual(#[from] rayo_visual::error::VisualError),

    #[error("assertion failed: {message}")]
    AssertionFailed { message: String },

    #[error("step '{step}' failed: {message}")]
    StepFailed { step: String, message: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}
