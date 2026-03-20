mod config;
mod state;

pub use config::UpdateConfig;
pub use state::{StateDir, UpdateMarker};

use axoupdater::{AxoUpdater, ReleaseSource, ReleaseSourceType};

/// Result of a successful update check.
pub enum UpdateOutcome {
    /// Updated to a new version. Takes effect on next startup.
    Updated { from: String, to: String },
    /// Already on the latest version.
    UpToDate,
    /// Skipped: rate limited, disabled, or lock held.
    Skipped,
}

/// Check for updates and apply if available.
///
/// This is the main entry point. Call from a `tokio::spawn` on startup.
/// Every error is handled internally — this only returns Err for truly
/// unexpected panics.
pub async fn check_and_update(
    config: &UpdateConfig,
    current_version: &str,
) -> Result<UpdateOutcome, Box<dyn std::error::Error + Send + Sync>> {
    if config.disabled {
        tracing::debug!("Auto-update disabled via RAYO_NO_UPDATE");
        return Ok(UpdateOutcome::Skipped);
    }

    let state = StateDir::new(config.state_dir.clone())?;

    // Rate limit
    let elapsed = state.seconds_since_last_check();
    if elapsed < config.check_interval_secs {
        tracing::debug!(
            "Update check skipped: checked {}s ago (interval: {}s)",
            elapsed,
            config.check_interval_secs
        );
        return Ok(UpdateOutcome::Skipped);
    }

    // File lock — skip if another instance is updating
    let _lock = match state.try_lock() {
        Some(lock) => lock,
        None => {
            tracing::debug!("Update check skipped: another instance holds the lock");
            return Ok(UpdateOutcome::Skipped);
        }
    };

    // Record that we're checking now
    if let Err(e) = state.write_last_check() {
        tracing::debug!("Failed to write last-check timestamp: {e}");
    }

    // Configure axoupdater
    let mut updater = AxoUpdater::new_for(&config.app_name);

    // Try loading install receipt first (created by cargo-dist installer)
    let has_receipt = updater.load_receipt().is_ok();

    if !has_receipt {
        // Manual config fallback for cargo install users
        updater.set_release_source(ReleaseSource {
            release_type: ReleaseSourceType::GitHub,
            owner: config.github_owner.clone(),
            name: config.github_repo.clone(),
            app_name: config.app_name.clone(),
        });

        // Derive install directory from current executable location.
        // Without this, axoupdater can't determine where to install and the
        // update silently fails with "not properly configured".
        // The installer script will also create an install receipt, so future
        // updates will use the receipt path and handle all binaries atomically.
        if let Ok(exe) = std::env::current_exe()
            && let Some(bin_dir) = exe.parent()
        {
            let install_dir = if bin_dir.file_name()
                == Some(std::ffi::OsStr::new("bin"))
            {
                bin_dir.parent().unwrap_or(bin_dir)
            } else {
                bin_dir
            };
            updater.set_install_dir(install_dir.to_string_lossy().as_ref());
        }
    }

    updater
        .set_current_version(current_version.parse()?)
        .map_err(|e| format!("Failed to set current version: {e}"))?;

    // Disable installer output (we're running in background)
    updater.disable_installer_output();

    // Check if update needed
    let update_needed = updater.is_update_needed().await.map_err(|e| {
        tracing::debug!("Update check failed: {e}");
        e
    })?;

    if !update_needed {
        tracing::debug!("rayo-mcp v{current_version} is up to date");
        return Ok(UpdateOutcome::UpToDate);
    }

    // Perform update
    tracing::debug!("Update available, downloading...");
    let result = updater.run().await.map_err(|e| {
        tracing::debug!("Update failed: {e}");
        e
    })?;

    match result {
        Some(result) => {
            let from = current_version.to_string();
            let to = result.new_version.to_string();

            // Write update marker for next startup
            let marker = UpdateMarker::new(&from, &to);
            if let Err(e) = state.write_marker(&marker) {
                tracing::debug!("Failed to write update marker: {e}");
            }

            tracing::debug!("Update downloaded: v{from} → v{to} (applies on next startup)");
            Ok(UpdateOutcome::Updated { from, to })
        }
        None => Ok(UpdateOutcome::UpToDate),
    }
}

/// Check for a post-update marker on startup. Call synchronously before spawning
/// the background update check.
///
/// Returns the action taken, if any.
pub fn handle_startup_marker(state_dir: &std::path::Path, current_version: &str) -> StartupAction {
    let state = match StateDir::new(state_dir.to_path_buf()) {
        Ok(s) => s,
        Err(_) => return StartupAction::None,
    };

    let marker = match state.read_marker() {
        Some(m) => m,
        None => return StartupAction::None,
    };

    if !marker.is_stale(current_version) {
        // We're running the updated version — success!
        if let Err(e) = state.clear_marker() {
            tracing::debug!("Failed to clear update marker: {e}");
        }
        return StartupAction::JustUpdated {
            from: marker.from_version,
            to: marker.to_version,
        };
    }

    // Marker is stale — the updated version crashed before clearing it.
    // This means we're running after a rollback (or the update didn't actually
    // replace the binary, e.g. cargo install users without cargo-dist).
    if marker.crash_count >= 2 {
        // Crash loop — disable auto-update
        if let Err(e) = state.clear_marker() {
            tracing::debug!("Failed to clear crash-loop marker: {e}");
        }
        return StartupAction::CrashLoopDetected {
            version: marker.to_version,
        };
    }

    // Increment crash count
    let mut updated_marker = marker.clone();
    updated_marker.crash_count += 1;
    if let Err(e) = state.write_marker(&updated_marker) {
        tracing::debug!("Failed to update crash count: {e}");
    }

    StartupAction::RollbackDetected {
        attempted_version: marker.to_version,
    }
}

/// What happened during startup marker check.
pub enum StartupAction {
    /// No marker found — normal startup.
    None,
    /// Successfully running the new version after an update.
    JustUpdated { from: String, to: String },
    /// The updated version crashed; we're running the old version.
    RollbackDetected { attempted_version: String },
    /// Multiple crashes detected; auto-update disabled.
    CrashLoopDetected { version: String },
}
