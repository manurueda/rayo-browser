//! MCP server setup and lifecycle.

use anyhow::Result;

pub async fn run() -> Result<()> {
    // TODO: Initialize chromiumoxide browser connection
    // TODO: Initialize rayo-core with profiler + rules
    // TODO: Start MCP server via rmcp with stdio transport
    // TODO: Register tools: navigate, observe, interact, batch, network, profile

    tracing::info!("rayo-mcp server starting on stdio...");

    // Placeholder: keep alive until stdin closes
    tokio::signal::ctrl_c().await?;

    Ok(())
}
