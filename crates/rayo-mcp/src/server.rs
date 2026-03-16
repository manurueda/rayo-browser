//! MCP server: wires rmcp with chromiumoxide via rayo-core.
//!
//! 7 tools, multi-tab architecture, network interception.

use std::sync::Arc;

use anyhow::Result;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParam, CallToolResult, Content, Implementation, ListToolsResult,
    PaginatedRequestParam, ServerCapabilities, ServerInfo, Tool, ToolsCapability,
};
use rmcp::service::RequestContext;
use rmcp::{Error as McpError, RoleServer};
use serde_json::json;
use tokio::sync::Mutex;

use rayo_core::network::NetworkInterceptor;
use rayo_core::tab_manager::TabManager;
use rayo_core::{RayoBrowser, RayoPage};
use rayo_profiler::Profiler;
use rayo_rules::{RayoRulesConfig, RuleEngine};

use crate::tools;

fn json_schema(v: serde_json::Value) -> Arc<serde_json::Map<String, serde_json::Value>> {
    match v {
        serde_json::Value::Object(map) => Arc::new(map),
        _ => Arc::new(serde_json::Map::new()),
    }
}

/// The rayo MCP server state.
#[derive(Clone)]
pub struct RayoServer {
    browser: Arc<Mutex<Option<RayoBrowser>>>,
    tabs: Arc<Mutex<TabManager>>,
    network: Arc<Mutex<NetworkInterceptor>>,
    profiler: Arc<Profiler>,
    rules: Arc<Mutex<RuleEngine>>,
}

impl Default for RayoServer {
    fn default() -> Self {
        Self::new()
    }
}

impl RayoServer {
    pub fn new() -> Self {
        let profiler = Profiler::new();
        let rules_config = RayoRulesConfig::load(std::path::Path::new(".rayo-rules"));
        Self {
            browser: Arc::new(Mutex::new(None)),
            tabs: Arc::new(Mutex::new(TabManager::new())),
            network: Arc::new(Mutex::new(NetworkInterceptor::new())),
            profiler: Arc::new(profiler),
            rules: Arc::new(Mutex::new(RuleEngine::new(rules_config))),
        }
    }

    /// Ensure browser is launched and at least one tab exists.
    async fn ensure_browser(&self) -> Result<(), McpError> {
        let mut browser_guard = self.browser.lock().await;
        if browser_guard.is_none() {
            tracing::info!("Launching Chrome browser...");
            let profiler = (*self.profiler).clone();
            let browser = RayoBrowser::launch_with_profiler(profiler)
                .await
                .map_err(|e| {
                    McpError::internal_error(format!("Failed to launch browser: {e}"), None)
                })?;
            *browser_guard = Some(browser);
        }

        let mut tabs_guard = self.tabs.lock().await;
        if tabs_guard.is_empty()
            && let Some(browser) = browser_guard.as_ref()
        {
            let page = browser.new_page().await.map_err(|e| {
                McpError::internal_error(format!("Failed to create page: {e}"), None)
            })?;
            // Wire passive Network domain monitoring for capture
            page.enable_network_monitoring(Arc::clone(&self.network))
                .await
                .map_err(|e| {
                    McpError::internal_error(
                        format!("Failed to enable network monitoring: {e}"),
                        None,
                    )
                })?;
            let tab_id = "tab-0".to_string();
            tabs_guard.add_tab(tab_id, page);
            tracing::debug!("Created initial tab");
        }
        Ok(())
    }

    /// Resolve the active page or a specific tab's page.
    /// Returns a reference-counted lock guard that must be held for the duration of the call.
    async fn resolve_page<'a>(
        tabs: &'a tokio::sync::MutexGuard<'a, TabManager>,
        tab_id: Option<&str>,
    ) -> Result<&'a RayoPage, McpError> {
        let page = match tab_id {
            Some(id) => tabs.get_page(id),
            None => tabs.active_page(),
        };
        page.ok_or_else(|| McpError::internal_error("No page available", None))
    }

    /// Handle tab management actions within rayo_navigate.
    async fn handle_tab_action(
        &self,
        action: &str,
        params: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<CallToolResult, McpError> {
        match action {
            "new_tab" => {
                let browser_guard = self.browser.lock().await;
                let browser = browser_guard
                    .as_ref()
                    .ok_or_else(|| McpError::internal_error("No browser", None))?;
                let page = browser.new_page().await.map_err(|e| {
                    McpError::internal_error(format!("Failed to create tab: {e}"), None)
                })?;
                // Wire passive Network domain monitoring for capture
                page.enable_network_monitoring(Arc::clone(&self.network))
                    .await
                    .map_err(|e| {
                        McpError::internal_error(
                            format!("Failed to enable network monitoring: {e}"),
                            None,
                        )
                    })?;

                let mut tabs = self.tabs.lock().await;
                let tab_id = format!("tab-{}", tabs.tab_count());
                tabs.add_tab(tab_id.clone(), page);
                tracing::info!(tab_id = %tab_id, "Created new tab");

                // Optionally navigate the new tab
                if let Some(url) = params.get("url").and_then(|v| v.as_str())
                    && let Some(page) = tabs.active_page()
                {
                    page.goto(url)
                        .await
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                }

                let url = if let Some(page) = tabs.active_page() {
                    page.url().await.unwrap_or_default()
                } else {
                    "about:blank".to_string()
                };

                Ok(CallToolResult::success(vec![Content::text(format!(
                    "New tab created: {tab_id}\nURL: {url}"
                ))]))
            }
            "close_tab" => {
                let tab_id = params.get("tab_id").and_then(|v| v.as_str());
                let mut tabs = self.tabs.lock().await;

                let removed = if let Some(id) = tab_id {
                    tabs.remove_tab(id).is_some()
                } else if let Some(id) = tabs.active_tab_id().map(String::from) {
                    tabs.remove_tab(&id).is_some()
                } else {
                    false
                };

                if removed {
                    Ok(CallToolResult::success(vec![Content::text("Tab closed")]))
                } else {
                    Err(McpError::invalid_params("Tab not found", None))
                }
            }
            "list_tabs" => {
                let tabs = self.tabs.lock().await;
                let mut info = Vec::new();
                for id in tabs.tab_ids() {
                    let active = tabs.active_tab_id() == Some(id);
                    let url = if let Some(page) = tabs.get_page(id) {
                        page.url().await.unwrap_or_default()
                    } else {
                        "unknown".to_string()
                    };
                    info.push(json!({
                        "tab_id": id,
                        "url": url,
                        "active": active,
                    }));
                }
                let json = serde_json::to_string(&info).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            "switch_tab" => {
                let tab_id = params
                    .get("tab_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        McpError::invalid_params("tab_id is required for switch_tab", None)
                    })?;
                let mut tabs = self.tabs.lock().await;
                if tabs.set_active(tab_id) {
                    let url = if let Some(page) = tabs.active_page() {
                        page.url().await.unwrap_or_default()
                    } else {
                        "unknown".to_string()
                    };
                    Ok(CallToolResult::success(vec![Content::text(format!(
                        "Switched to tab {tab_id}\nURL: {url}"
                    ))]))
                } else {
                    Err(McpError::invalid_params(
                        format!("Tab not found: {tab_id}"),
                        None,
                    ))
                }
            }
            _ => Err(McpError::invalid_params(
                format!("Unknown tab action: {action}"),
                None,
            )),
        }
    }

    fn tool_definitions() -> Vec<Tool> {
        vec![
            Tool::new(
                "rayo_navigate",
                "Navigate the browser. Actions: goto (requires url), reload, back, forward. Tab actions: new_tab (optional url), close_tab (optional tab_id), list_tabs, switch_tab (requires tab_id). Returns page_map after goto.",
                json_schema(json!({
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["goto", "reload", "back", "forward", "new_tab", "close_tab", "list_tabs", "switch_tab"] },
                        "url": { "type": "string", "description": "URL to navigate to (required for goto, optional for new_tab)" },
                        "tab_id": { "type": "string", "description": "Tab ID for tab operations" }
                    },
                    "required": ["action"]
                })),
            ),
            Tool::new(
                "rayo_observe",
                "Observe the page. Modes: page_map (default, ~500 tokens, structured — supports selector to scope to a subtree), text (raw text — supports selector + max_elements), screenshot (base64 JPEG). Optional tab_id.",
                json_schema(json!({
                    "type": "object",
                    "properties": {
                        "mode": { "type": "string", "enum": ["page_map", "text", "screenshot"], "default": "page_map" },
                        "selector": { "type": "string", "description": "CSS selector to scope observation" },
                        "max_elements": { "type": "integer", "description": "Max elements for text mode with selector (default: 50)", "default": 50 },
                        "full_page": { "type": "boolean", "default": false },
                        "tab_id": { "type": "string", "description": "Tab ID (default: active tab)" }
                    }
                })),
            ),
            Tool::new(
                "rayo_interact",
                "Interact with an element. Use id from page_map or CSS selector. Actions: click, type (requires value), press (requires value — key name like \"Enter\", \"Tab\", \"Escape\", \"ArrowDown\"), select (requires value), scroll. Optional tab_id.",
                json_schema(json!({
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["click", "type", "press", "select", "scroll"] },
                        "id": { "type": "integer", "description": "Element ID from page_map" },
                        "selector": { "type": "string", "description": "CSS selector (alternative to id)" },
                        "value": { "type": "string", "description": "Text to type or option to select" },
                        "tab_id": { "type": "string", "description": "Tab ID (default: active tab)" }
                    },
                    "required": ["action"]
                })),
            ),
            Tool::new(
                "rayo_batch",
                "Execute multiple actions in one call. Each action: {action, id/selector, value}. Returns array of results. 5-7x faster than individual calls. Optional tab_id.",
                json_schema(json!({
                    "type": "object",
                    "properties": {
                        "actions": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "action": { "type": "string", "enum": ["click", "type", "press", "select", "goto", "screenshot", "wait_for", "scroll"] },
                                    "id": { "type": "integer" },
                                    "selector": { "type": "string" },
                                    "value": { "type": "string" },
                                    "key": { "type": "string", "description": "Key name for press action (e.g. Enter, Tab, Escape, ArrowDown)" },
                                    "url": { "type": "string" },
                                    "full_page": { "type": "boolean" },
                                    "timeout_ms": { "type": "integer" },
                                    "x": { "type": "integer" },
                                    "y": { "type": "integer" }
                                },
                                "required": ["action"]
                            }
                        },
                        "abort_on_failure": { "type": "boolean", "description": "Stop batch on first failure (default: false)", "default": false },
                        "tab_id": { "type": "string", "description": "Tab ID (default: active tab)" }
                    },
                    "required": ["actions"]
                })),
            ),
            Tool::new(
                "rayo_cookie",
                "Manage browser cookies. Actions: set (inject cookies), get (read, optional domain filter), clear (delete, optional domain filter), save (export to JSON file), load (import from JSON file), import (read cookies from a real browser — requires browser: chrome/arc/brave/edge/chromium, optional domain filter, optional profile). Optional tab_id.",
                json_schema(json!({
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["set", "get", "clear", "save", "load", "import"] },
                        "cookies": {
                            "type": "array",
                            "description": "Cookies to set (required for 'set' action)",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "name": { "type": "string" },
                                    "value": { "type": "string" },
                                    "domain": { "type": "string" },
                                    "path": { "type": "string" },
                                    "url": { "type": "string" },
                                    "secure": { "type": "boolean" },
                                    "httpOnly": { "type": "boolean" },
                                    "sameSite": { "type": "string", "enum": ["Strict", "Lax", "None"] },
                                    "expires": { "type": "number" }
                                },
                                "required": ["name", "value"]
                            }
                        },
                        "domain": { "type": "string", "description": "Filter by domain (for 'get', 'clear', 'save', and 'import' actions)" },
                        "path": { "type": "string", "description": "File path for save/load actions (JSON format)" },
                        "browser": { "type": "string", "enum": ["chrome", "arc", "brave", "edge", "chromium"], "description": "Browser to import cookies from (required for 'import' action)" },
                        "profile": { "type": "string", "description": "Browser profile name for import (default: 'Default')" },
                        "tab_id": { "type": "string", "description": "Tab ID (default: active tab)" }
                    },
                    "required": ["action"]
                })),
            ),
            Tool::new(
                "rayo_network",
                "Network operations. Modes: capture (start capturing requests), requests (get captured, optional url_pattern filter), block (block URLs matching url_pattern), mock (mock responses: url_pattern + response {status, body, headers}), clear (reset all).",
                json_schema(json!({
                    "type": "object",
                    "properties": {
                        "mode": { "type": "string", "enum": ["capture", "requests", "block", "mock", "clear"] },
                        "url_pattern": { "type": "string", "description": "URL pattern (glob: * matches anything)" },
                        "resource_type": { "type": "string", "description": "Filter by resource type (script, stylesheet, image, document)" },
                        "response": {
                            "type": "object",
                            "description": "Mock response (for mock mode)",
                            "properties": {
                                "status": { "type": "integer", "default": 200 },
                                "body": { "type": "string" },
                                "headers": { "type": "object" }
                            }
                        }
                    },
                    "required": ["mode"]
                })),
            ),
            Tool::new(
                "rayo_profile",
                "Get profiling results. Formats: ai_summary (default, token-efficient), json, markdown, chrome_trace.",
                json_schema(json!({
                    "type": "object",
                    "properties": {
                        "format": { "type": "string", "enum": ["ai_summary", "json", "markdown", "chrome_trace"], "default": "ai_summary" }
                    }
                })),
            ),
        ]
    }
}

#[allow(clippy::manual_async_fn)]
impl ServerHandler for RayoServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: Default::default(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    ..Default::default()
                }),
                ..Default::default()
            },
            server_info: Implementation {
                name: "rayo-browser".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            instructions: Some(rayo_rules::defaults::rules_as_agent_text()),
        }
    }

    fn list_tools(
        &self,
        _request: PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        async move {
            Ok(ListToolsResult {
                tools: Self::tool_definitions(),
                next_cursor: None,
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        async move {
            self.ensure_browser().await?;

            let tool_name = request.name.as_ref();
            let params = request.arguments.unwrap_or_default();

            let start = std::time::Instant::now();
            tracing::info!(tool = tool_name, "Tool call");

            // Extract optional tab_id for page-based tools
            let tab_id = params.get("tab_id").and_then(|v| v.as_str());

            let result = match tool_name {
                "rayo_navigate" => {
                    let action = params
                        .get("action")
                        .and_then(|v| v.as_str())
                        .unwrap_or("goto");

                    // Tab management actions are handled separately
                    match action {
                        "new_tab" | "close_tab" | "list_tabs" | "switch_tab" => {
                            self.handle_tab_action(action, &params).await
                        }
                        _ => {
                            let tabs = self.tabs.lock().await;
                            let page = Self::resolve_page(&tabs, tab_id).await?;
                            tools::handle_navigate(page, &params).await
                        }
                    }
                }
                "rayo_observe" => {
                    let tabs = self.tabs.lock().await;
                    let page = Self::resolve_page(&tabs, tab_id).await?;
                    tools::handle_observe(page, &params, &self.rules).await
                }
                "rayo_interact" => {
                    let tabs = self.tabs.lock().await;
                    let page = Self::resolve_page(&tabs, tab_id).await?;
                    tools::handle_interact(page, &params, &self.rules).await
                }
                "rayo_batch" => {
                    let tabs = self.tabs.lock().await;
                    let page = Self::resolve_page(&tabs, tab_id).await?;
                    tools::handle_batch(page, &params).await
                }
                "rayo_cookie" => {
                    let tabs = self.tabs.lock().await;
                    let page = Self::resolve_page(&tabs, tab_id).await?;
                    tools::handle_cookie(page, &params).await
                }
                "rayo_network" => {
                    let tabs = self.tabs.lock().await;
                    let page = Self::resolve_page(&tabs, tab_id).await?;
                    tools::handle_network(page, &self.network, &params).await
                }
                "rayo_profile" => tools::handle_profile(&self.profiler, &params).await,
                _ => Err(McpError::invalid_request(
                    format!("Unknown tool: {tool_name}"),
                    None,
                )),
            };

            let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
            tracing::info!(
                tool = tool_name,
                duration_ms = duration_ms,
                success = result.is_ok(),
                "Tool complete"
            );

            // Attach _rayo profiling metadata
            let violations = self.rules.lock().await.drain_violations();

            match result {
                Ok(mut call_result) => {
                    if !violations.is_empty() || duration_ms > 1.0 {
                        let rayo_meta = json!({
                            "_rayo": {
                                "durationMs": (duration_ms * 10.0).round() / 10.0,
                                "violations": violations,
                            }
                        });
                        call_result.content.push(Content::text(
                            serde_json::to_string(&rayo_meta).unwrap_or_default(),
                        ));
                    }
                    Ok(call_result)
                }
                Err(e) => {
                    tracing::error!(tool = tool_name, error = %e, "Tool error");
                    Err(e)
                }
            }
        }
    }
}

pub async fn run() -> Result<()> {
    let server = RayoServer::new();
    let transport = rmcp::transport::io::stdio();

    tracing::info!(
        "rayo-mcp v{} starting on stdio (7 tools)",
        env!("CARGO_PKG_VERSION")
    );

    let service = rmcp::serve_server(server, transport)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start MCP server: {e}"))?;

    service.waiting().await?;
    Ok(())
}
