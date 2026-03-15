//! rayo-mcp: The fastest MCP browser automation server.
//!
//! Single binary. Connects to Chrome via CDP (chromiumoxide).
//! 6 tools, ~1,500 tokens. vs Playwright MCP's 20+ tools at ~13,700 tokens.

mod server;
mod tools;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    tracing::info!("rayo-mcp v{}", env!("CARGO_PKG_VERSION"));

    server::run().await
}
