use std::path::PathBuf;

use crate::state::StateDir;

/// Configuration for the auto-updater.
pub struct UpdateConfig {
    /// Disable auto-update entirely.
    pub disabled: bool,
    /// Minimum seconds between version checks.
    pub check_interval_secs: u64,
    /// GitHub owner/repo (e.g. "manurueda/rayo-browser").
    pub github_owner: String,
    pub github_repo: String,
    /// Binary name to look for in release assets.
    pub app_name: String,
    /// State directory path.
    pub state_dir: PathBuf,
}

impl UpdateConfig {
    /// Load config from environment variables with sensible defaults.
    pub fn from_env() -> Self {
        let disabled = std::env::var("RAYO_NO_UPDATE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let check_interval_secs = std::env::var("RAYO_UPDATE_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3600);

        Self {
            disabled,
            check_interval_secs,
            github_owner: "manurueda".to_string(),
            github_repo: "rayo-browser".to_string(),
            app_name: "rayo-mcp".to_string(),
            state_dir: StateDir::default_path(),
        }
    }
}
