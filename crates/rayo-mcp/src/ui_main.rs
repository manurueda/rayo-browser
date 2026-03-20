#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let version = env!("CARGO_PKG_VERSION");

    // Check for post-update marker
    let config = rayo_updater::UpdateConfig::from_env();
    match rayo_updater::handle_startup_marker(&config.state_dir, version) {
        rayo_updater::StartupAction::JustUpdated { from, to } => {
            eprintln!("⚡ rayo updated v{from} → v{to}");
        }
        rayo_updater::StartupAction::RollbackDetected {
            attempted_version, ..
        } => {
            eprintln!("⚠ Update to v{attempted_version} failed — rolled back to v{version}");
        }
        rayo_updater::StartupAction::CrashLoopDetected { version: ver } => {
            eprintln!(
                "✗ Update crash loop detected (v{ver}) — auto-update disabled. \
                 Reinstall: curl --proto '=https' --tlsv1.2 -LsSf \
                 https://github.com/manurueda/rayo-browser/releases/latest/download/rayo-mcp-installer.sh | sh"
            );
        }
        rayo_updater::StartupAction::None => {}
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
                Err(_) => {}
            }
        });
    }

    rayo_ui::run().await
}
