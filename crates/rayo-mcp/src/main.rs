//! rayo-mcp: The fastest MCP browser automation server.
//!
//! Single binary. Connects to Chrome via CDP (chromiumoxide).
//! 5 tools, ~1,500 tokens. vs Playwright MCP's 20+ tools at ~13,700 tokens.

use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");

    // Handle --version flag before any other setup
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("rayo-mcp {version}");
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    // Check for post-update marker (fast, synchronous file read)
    let config = rayo_updater::UpdateConfig::from_env();
    match rayo_updater::handle_startup_marker(&config.state_dir, version) {
        rayo_updater::StartupAction::JustUpdated { from, to } => {
            tracing::info!("rayo-mcp v{to} (updated from v{from})");
        }
        rayo_updater::StartupAction::RollbackDetected {
            attempted_version, ..
        } => {
            tracing::warn!("Update to v{attempted_version} failed — rolled back to v{version}");
        }
        rayo_updater::StartupAction::CrashLoopDetected { version: ver } => {
            tracing::error!(
                "Update crash loop detected (v{ver} failed repeatedly) — auto-update disabled. \
                 Manually update with: curl --proto '=https' --tlsv1.2 -LsSf \
                 https://github.com/manurueda/rayo-browser/releases/latest/download/rayo-mcp-installer.sh | sh"
            );
        }
        rayo_updater::StartupAction::None => {
            tracing::info!("rayo-mcp v{version}");
        }
    }

    // Spawn background update check (non-blocking)
    if !config.disabled {
        let version_owned = version.to_string();
        tokio::spawn(async move {
            match rayo_updater::check_and_update(&config, &version_owned).await {
                Ok(rayo_updater::UpdateOutcome::Updated { from, to }) => {
                    eprintln!("⚡ rayo updated v{from} → v{to} (restart to apply)");
                }
                Ok(_) => {}
                Err(e) => tracing::debug!("Update check failed: {e}"),
            }
        });
    }

    rayo_mcp::server::run().await
}
