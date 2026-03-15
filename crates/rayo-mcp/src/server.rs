//! MCP server: wires rmcp with chromiumoxide via rayo-core.

use std::sync::Arc;

use anyhow::Result;
use rmcp::model::{
    CallToolResult, Content, Implementation, ListToolsResult,
    PaginatedRequestParam, ServerCapabilities, ServerInfo, Tool, ToolsCapability,
    CallToolRequestParam,
};
use rmcp::handler::server::ServerHandler;
use rmcp::service::RequestContext;
use rmcp::{Error as McpError, RoleServer};
use serde_json::json;
use tokio::sync::Mutex;

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
    page: Arc<Mutex<Option<RayoPage>>>,
    profiler: Arc<Profiler>,
    rules: Arc<Mutex<RuleEngine>>,
}

impl RayoServer {
    pub fn new() -> Self {
        let profiler = Profiler::new();
        let rules_config = RayoRulesConfig::load(std::path::Path::new(".rayo-rules"));
        Self {
            browser: Arc::new(Mutex::new(None)),
            page: Arc::new(Mutex::new(None)),
            profiler: Arc::new(profiler),
            rules: Arc::new(Mutex::new(RuleEngine::new(rules_config))),
        }
    }

    /// Ensure browser is launched and a page exists.
    async fn ensure_browser(&self) -> Result<(), McpError> {
        let mut browser_guard = self.browser.lock().await;
        if browser_guard.is_none() {
            let browser = RayoBrowser::launch()
                .await
                .map_err(|e| McpError::internal_error(format!("Failed to launch browser: {e}"), None))?;
            *browser_guard = Some(browser);
        }

        let mut page_guard = self.page.lock().await;
        if page_guard.is_none() {
            if let Some(browser) = browser_guard.as_ref() {
                let page = browser
                    .new_page()
                    .await
                    .map_err(|e| McpError::internal_error(format!("Failed to create page: {e}"), None))?;
                *page_guard = Some(page);
            }
        }
        Ok(())
    }

    fn tool_definitions() -> Vec<Tool> {
        vec![
            Tool::new(
                "rayo_navigate",
                "Navigate the browser. Actions: goto (requires url), reload, back, forward.",
                json_schema(json!({
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["goto", "reload", "back", "forward"] },
                        "url": { "type": "string", "description": "URL to navigate to (required for goto)" }
                    },
                    "required": ["action"]
                })),
            ),
            Tool::new(
                "rayo_observe",
                "Observe the page. Modes: page_map (default, ~500 tokens, structured), text (raw text), screenshot (base64 PNG).",
                json_schema(json!({
                    "type": "object",
                    "properties": {
                        "mode": { "type": "string", "enum": ["page_map", "text", "screenshot"], "default": "page_map" },
                        "selector": { "type": "string", "description": "CSS selector to scope observation" },
                        "full_page": { "type": "boolean", "default": false }
                    }
                })),
            ),
            Tool::new(
                "rayo_interact",
                "Interact with an element. Use id from page_map or CSS selector. Actions: click, type (requires value), select (requires value), scroll.",
                json_schema(json!({
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["click", "type", "select", "scroll"] },
                        "id": { "type": "integer", "description": "Element ID from page_map" },
                        "selector": { "type": "string", "description": "CSS selector (alternative to id)" },
                        "value": { "type": "string", "description": "Text to type or option to select" }
                    },
                    "required": ["action"]
                })),
            ),
            Tool::new(
                "rayo_batch",
                "Execute multiple actions in one call. Each action: {action, id/selector, value}. Returns array of results. 5-7x faster than individual calls.",
                json_schema(json!({
                    "type": "object",
                    "properties": {
                        "actions": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "action": { "type": "string", "enum": ["click", "type", "select", "goto", "screenshot", "wait_for", "scroll"] },
                                    "id": { "type": "integer" },
                                    "selector": { "type": "string" },
                                    "value": { "type": "string" },
                                    "url": { "type": "string" },
                                    "full_page": { "type": "boolean" },
                                    "timeout_ms": { "type": "integer" },
                                    "x": { "type": "integer" },
                                    "y": { "type": "integer" }
                                },
                                "required": ["action"]
                            }
                        }
                    },
                    "required": ["actions"]
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
            Tool::new(
                "rayo_cookie",
                "Manage browser cookies. Actions: set (inject cookies for auth), get (read cookies, optional domain filter), clear (delete cookies, optional domain filter).",
                json_schema(json!({
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["set", "get", "clear"] },
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
                                    "url": { "type": "string", "description": "URL to associate with the cookie (alternative to domain)" },
                                    "secure": { "type": "boolean" },
                                    "httpOnly": { "type": "boolean" },
                                    "sameSite": { "type": "string", "enum": ["Strict", "Lax", "None"] },
                                    "expires": { "type": "number", "description": "Expiration as Unix timestamp (seconds)" }
                                },
                                "required": ["name", "value"]
                            }
                        },
                        "domain": { "type": "string", "description": "Filter by domain (for 'get' and 'clear' actions)" }
                    },
                    "required": ["action"]
                })),
            ),
        ]
    }
}

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

            let result = match tool_name {
                "rayo_navigate" => tools::handle_navigate(&self.page, &params).await,
                "rayo_observe" => tools::handle_observe(&self.page, &params, &self.rules).await,
                "rayo_interact" => tools::handle_interact(&self.page, &params, &self.rules).await,
                "rayo_batch" => tools::handle_batch(&self.page, &params).await,
                "rayo_profile" => tools::handle_profile(&self.profiler).await,
                "rayo_cookie" => tools::handle_cookie(&self.page, &params).await,
                _ => Err(McpError::invalid_request(
                    format!("Unknown tool: {tool_name}"),
                    None,
                )),
            };

            let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

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
                Err(e) => Err(e),
            }
        }
    }
}

pub async fn run() -> Result<()> {
    let server = RayoServer::new();
    let transport = rmcp::transport::io::stdio();

    tracing::info!("rayo-mcp v{} starting on stdio", env!("CARGO_PKG_VERSION"));

    let service = rmcp::serve_server(server, transport)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start MCP server: {e}"))?;

    service.waiting().await?;
    Ok(())
}
