# Auto-Update: Design

## Crate: rayo-updater

### Dependencies
- `axoupdater` — cargo-dist's official update library (handles platform detection, download, checksums)
- `reqwest` — HTTP client (pulled in by axoupdater, used for version check)
- `semver` — Version comparison
- `serde` + `serde_json` — State file serialization
- `tracing` — Logging
- `tempfile` — Safe temp file creation for downloads
- `fs2` — Cross-platform advisory file locking (flock)

### Public API

```rust
/// Check for updates and apply if available. Non-blocking, non-fatal.
/// Returns Ok(Some(version)) if updated, Ok(None) if up-to-date/skipped,
/// Err only for unexpected panics (all expected errors handled internally).
pub async fn check_and_update(config: &UpdateConfig) -> Result<Option<String>>;

pub struct UpdateConfig {
    /// Disable auto-update entirely (default: false, overridden by RAYO_NO_UPDATE=1)
    pub disabled: bool,
    /// Minimum seconds between checks (default: 3600)
    pub check_interval_secs: u64,
    /// GitHub owner/repo (default: "manurueda/rayo-browser")
    pub github_repo: String,
    /// Binary name to look for in release assets (default: "rayo-mcp")
    pub binary_name: String,
    /// State directory (default: ~/.rayo)
    pub state_dir: PathBuf,
}
```

### State Files (in ~/.rayo/)

```
~/.rayo/
├── last-check          # Unix timestamp of last version check
├── update-marker.json  # {"from": "0.1.0", "to": "0.2.0", "at": "2026-03-16T..."}
├── update.lock         # Advisory file lock (flock)
└── .rayo-mcp.backup    # Previous binary (lives next to the installed binary, not in ~/.rayo)
```

### Update Marker Lifecycle

```
  ┌─────────────┐     ┌──────────────┐     ┌────────────────┐
  │ Replace      │────▶│ Write marker │────▶│ Next startup   │
  │ binary       │     │ {from, to}   │     │ reads marker   │
  └─────────────┘     └──────────────┘     └────────────────┘
                                                   │
                                     ┌─────────────┴──────────┐
                                     │                        │
                              ┌──────▼──────┐         ┌──────▼──────┐
                              │ Startup OK  │         │ Startup     │
                              │ Clear marker│         │ crashes     │
                              │ Log updated │         │ Marker stale│
                              └─────────────┘         └──────┬──────┘
                                                             │
                                                      ┌──────▼──────┐
                                                      │ Next start: │
                                                      │ See stale   │
                                                      │ marker →    │
                                                      │ ROLLBACK    │
                                                      └─────────────┘
```

### Crash Loop Prevention
Track consecutive crash count in update-marker.json. If `crash_count >= 2`:
- Rollback to backup
- Set `auto_update_disabled_reason: "crash_loop"` in marker
- Log error with instructions to manually update or file a bug

### Log Levels
- `debug` — Check skipped (rate limit), check result, download progress, skip reasons
- `info` — "rayo-mcp v0.2.0 (updated from v0.1.0)", "rayo-mcp v0.1.0 (up to date)"
- `warn` — Permission denied (with fix suggestion), rollback triggered
- `error` — Crash loop detected, auto-update disabled

### Binary Replacement Strategy
axoupdater handles the actual download and replacement. Our layer adds:
1. Pre-replace: acquire flock, backup current binary
2. Delegate to axoupdater: download + verify + replace
3. Post-replace: write update marker, release lock

### Platform Detection
axoupdater auto-detects the current platform and selects the correct release asset.
cargo-dist builds for: `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`.

### Integration Point in rayo-mcp

```rust
// main.rs — after tracing init, before server start
let version = env!("CARGO_PKG_VERSION");

// Check for post-update marker (synchronous, fast file read)
if let Some(marker) = rayo_updater::check_update_marker(&state_dir) {
    if marker.is_stale() {
        rayo_updater::rollback(&state_dir);
        tracing::warn!("Rolled back from v{} to previous version", marker.to);
    } else {
        tracing::info!("rayo-mcp v{} (updated from v{})", marker.to, marker.from);
        rayo_updater::clear_marker(&state_dir);
    }
}

tracing::info!("rayo-mcp v{}", version);

// Background update check (non-blocking)
let config = rayo_updater::UpdateConfig::from_env();
if !config.disabled {
    tokio::spawn(async move {
        if let Err(e) = rayo_updater::check_and_update(&config).await {
            tracing::debug!("Update check failed: {e}");
        }
    });
}

// Start MCP server (immediate)
rayo_mcp::server::run().await
```
