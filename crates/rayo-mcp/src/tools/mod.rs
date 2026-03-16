//! MCP tool handlers.
//!
//! 7 tools, ~2,000 tokens total tool description.
//! Handlers receive a resolved &RayoPage — tab resolution is done by the server.

use std::sync::Arc;

use rayo_core::RayoPage;
use rayo_core::network::NetworkInterceptor;
use rayo_profiler::Profiler;
use rayo_rules::RuleEngine;
use rmcp::Error as McpError;
use rmcp::model::{CallToolResult, Content};
use serde_json::Value;
use tokio::sync::Mutex;

/// Helper to convert RayoError or similar into McpError.
fn internal_err(e: impl std::fmt::Display) -> McpError {
    McpError::internal_error(e.to_string(), None)
}

pub async fn handle_navigate(
    page: &RayoPage,
    params: &serde_json::Map<String, Value>,
) -> Result<CallToolResult, McpError> {
    let action = params
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("goto");

    match action {
        "goto" => {
            let url = params
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| McpError::invalid_params("url is required for goto", None))?;
            let wait_until = params
                .get("wait_until")
                .and_then(|v| v.as_str())
                .unwrap_or("load");

            // Use transparent auto-auth: detects auth walls, imports cookies
            // from the user's real browser, and retries navigation if needed.
            // Zero overhead when no auth wall is detected.
            let map = page.goto_with_auto_auth(url).await.map_err(internal_err)?;

            // Handle wait_until conditions beyond the default page load.
            // "load" and "domcontentloaded" are already satisfied by goto()
            // which waits for the page load event.
            if wait_until == "networkidle" {
                // Wait for 500ms of no new network activity.
                // Uses Performance API to detect ongoing resource fetches.
                page.wait_for_network_idle(500, 5000)
                    .await
                    .map_err(internal_err)?;
            }

            // Auto-return page_map after navigation (delight feature)
            // page_map already contains title and URL — no separate CDP calls needed
            // If we waited for networkidle, re-fetch the page_map since it may have changed.
            let map = if wait_until == "networkidle" {
                page.page_map(None).await.map_err(internal_err)?
            } else {
                map
            };
            let json = serde_json::to_string(&map).unwrap_or_default();
            let waited = if wait_until != "load" {
                format!(" (waited for {wait_until})")
            } else {
                String::new()
            };
            let content = vec![
                Content::text(format!(
                    "Navigated to {}\nTitle: {}{}",
                    map.url, map.title, waited
                )),
                Content::text(format!("\n--- page_map ---\n{json}")),
            ];
            Ok(CallToolResult::success(content))
        }
        "reload" => {
            page.reload().await.map_err(internal_err)?;
            Ok(CallToolResult::success(vec![Content::text(
                "Page reloaded",
            )]))
        }
        "back" => {
            // Check if there's history to go back to
            let history_len = page
                .evaluate("history.length")
                .await
                .unwrap_or(serde_json::json!(1));
            let len = history_len.as_u64().unwrap_or(1);
            if len <= 1 {
                return Ok(CallToolResult::success(vec![Content::text(
                    "No history to go back to",
                )]));
            }
            page.evaluate("history.back()")
                .await
                .map_err(internal_err)?;
            // Wait briefly for navigation to settle
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let url = page.url().await.unwrap_or_default();
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Navigated back to {url}"
            ))]))
        }
        "forward" => {
            page.evaluate("history.forward()")
                .await
                .map_err(internal_err)?;
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let url = page.url().await.unwrap_or_default();
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Navigated forward to {url}"
            ))]))
        }
        _ => Err(McpError::invalid_params(
            format!(
                "Unknown navigate action: {action}. Tab actions (new_tab, close_tab, list_tabs, switch_tab) are handled by the server."
            ),
            None,
        )),
    }
}

pub async fn handle_observe(
    page: &RayoPage,
    params: &serde_json::Map<String, Value>,
    rules: &Arc<Mutex<RuleEngine>>,
) -> Result<CallToolResult, McpError> {
    let mode = params
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("page_map");

    match mode {
        "page_map" => {
            let selector = params.get("selector").and_then(|v| v.as_str());
            let map = page.page_map(selector).await.map_err(internal_err)?;
            let json = serde_json::to_string(&map).unwrap_or_default();
            Ok(CallToolResult::success(vec![Content::text(json)]))
        }
        "text" => {
            let selector = params.get("selector").and_then(|v| v.as_str());
            let max_elements = params
                .get("max_elements")
                .and_then(|v| v.as_u64())
                .unwrap_or(50) as usize;
            let text = page
                .text_content(selector, max_elements)
                .await
                .map_err(internal_err)?;
            Ok(CallToolResult::success(vec![Content::text(text)]))
        }
        "screenshot" => {
            let mut engine = rules.lock().await;
            engine.check_screenshot();
            engine.check_page_map_preference();
            let (screenshots_remaining, reset_in_ms) = engine.screenshot_rate_info();
            drop(engine);
            let full_page = params
                .get("full_page")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let b64 = page.screenshot(full_page).await.map_err(internal_err)?;
            let meta = serde_json::json!({
                "_rayo": {
                    "screenshots_remaining": screenshots_remaining,
                    "reset_in_ms": reset_in_ms
                }
            });
            Ok(CallToolResult::success(vec![
                Content::image(b64, RayoPage::screenshot_mime()),
                Content::text(serde_json::to_string(&meta).unwrap_or_default()),
            ]))
        }
        _ => Err(McpError::invalid_params(
            format!("Unknown observe mode: {mode}"),
            None,
        )),
    }
}

pub async fn handle_interact(
    page: &RayoPage,
    params: &serde_json::Map<String, Value>,
    rules: &Arc<Mutex<RuleEngine>>,
) -> Result<CallToolResult, McpError> {
    let action = params
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("action is required", None))?;

    // Check selector rules
    if let Some(sel) = params.get("selector").and_then(|v| v.as_str()) {
        rules.lock().await.check_selector(sel);
    }

    let id = params
        .get("id")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let selector = params.get("selector").and_then(|v| v.as_str());
    let value = params.get("value").and_then(|v| v.as_str());

    let msg = match action {
        "click" => {
            page.click(selector, id).await.map_err(internal_err)?;
            "Clicked".to_string()
        }
        "hover" => {
            page.hover(selector, id).await.map_err(internal_err)?;
            "Hovered".to_string()
        }
        "type" => {
            let text = value
                .ok_or_else(|| McpError::invalid_params("value is required for type", None))?;
            page.type_text(selector, id, text, true)
                .await
                .map_err(internal_err)?;
            format!("Typed: {text}")
        }
        "select" => {
            let val = value
                .ok_or_else(|| McpError::invalid_params("value is required for select", None))?;
            page.select_option(selector, id, val)
                .await
                .map_err(internal_err)?;
            format!("Selected: {val}")
        }
        "press" => {
            let key = value.ok_or_else(|| {
                McpError::invalid_params(
                    "value is required for press (key name, e.g. \"Enter\", \"Tab\", \"Escape\")",
                    None,
                )
            })?;
            page.press_key(selector, id, key)
                .await
                .map_err(internal_err)?;
            format!("Pressed: {key}")
        }
        "scroll" => {
            if let Some(sel) = selector {
                let js = format!(
                    "document.querySelector({}).scrollIntoView({{block:'center'}})",
                    serde_json::to_string(sel).unwrap()
                );
                page.evaluate(&js).await.map_err(internal_err)?;
            } else if let Some(element_id) = id {
                page.click(None, Some(element_id))
                    .await
                    .map_err(internal_err)?;
            }
            "Scrolled".to_string()
        }
        _ => {
            return Err(McpError::invalid_params(
                format!("Unknown interact action: {action}"),
                None,
            ));
        }
    };

    Ok(CallToolResult::success(vec![Content::text(msg)]))
}

pub async fn handle_batch(
    page: &RayoPage,
    params: &serde_json::Map<String, Value>,
) -> Result<CallToolResult, McpError> {
    let actions_value = params
        .get("actions")
        .ok_or_else(|| McpError::invalid_params("actions array is required", None))?;

    let actions: Vec<rayo_core::batch::BatchAction> = serde_json::from_value(actions_value.clone())
        .map_err(|e| McpError::invalid_params(format!("Invalid actions: {e}"), None))?;

    let abort_on_failure = params
        .get("abort_on_failure")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let result = page
        .execute_batch(actions, abort_on_failure)
        .await
        .map_err(internal_err)?;

    let json = serde_json::to_string(&result).unwrap_or_default();
    // Auto-return page_map so LLM doesn't need a separate observe call
    let mut content = vec![Content::text(json)];
    if let Ok(map) = page.page_map(None).await {
        let map_json = serde_json::to_string(&map).unwrap_or_default();
        content.push(Content::text(format!("\n--- page_map ---\n{map_json}")));
    }
    Ok(CallToolResult::success(content))
}

pub async fn handle_cookie(
    page: &RayoPage,
    params: &serde_json::Map<String, Value>,
) -> Result<CallToolResult, McpError> {
    let action = params
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("action is required", None))?;

    match action {
        "set" => {
            let cookies_value = params.get("cookies").ok_or_else(|| {
                McpError::invalid_params("cookies array is required for set", None)
            })?;
            let cookie_entries: Vec<Value> = serde_json::from_value(cookies_value.clone())
                .map_err(|e| {
                    McpError::invalid_params(format!("Invalid cookies array: {e}"), None)
                })?;

            let mut cookies = Vec::with_capacity(cookie_entries.len());
            for entry in &cookie_entries {
                let name = entry
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::invalid_params("Each cookie requires a name", None))?;
                let value = entry.get("value").and_then(|v| v.as_str()).ok_or_else(|| {
                    McpError::invalid_params("Each cookie requires a value", None)
                })?;

                cookies.push(rayo_core::SetCookie {
                    name: name.to_string(),
                    value: value.to_string(),
                    domain: entry
                        .get("domain")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    path: entry.get("path").and_then(|v| v.as_str()).map(String::from),
                    url: entry.get("url").and_then(|v| v.as_str()).map(String::from),
                    secure: entry.get("secure").and_then(|v| v.as_bool()),
                    http_only: entry.get("httpOnly").and_then(|v| v.as_bool()),
                    same_site: entry.get("sameSite").and_then(|v| v.as_str()).and_then(
                        |s| match s {
                            "Strict" => Some(rayo_core::SameSite::Strict),
                            "Lax" => Some(rayo_core::SameSite::Lax),
                            "None" => Some(rayo_core::SameSite::None),
                            _ => None,
                        },
                    ),
                    expires: entry.get("expires").and_then(|v| v.as_f64()),
                });
            }

            let count = cookies.len();
            page.set_cookies(cookies).await.map_err(internal_err)?;
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Set {count} cookie(s)"
            ))]))
        }
        "get" => {
            let cookies = page.get_cookies().await.map_err(internal_err)?;
            let domain_filter = params.get("domain").and_then(|v| v.as_str());
            let filtered: Vec<_> = if let Some(domain) = domain_filter {
                cookies
                    .into_iter()
                    .filter(|c| c.domain.contains(domain))
                    .collect()
            } else {
                cookies
            };
            let json = serde_json::to_string(&filtered).unwrap_or_default();
            Ok(CallToolResult::success(vec![Content::text(json)]))
        }
        "clear" => {
            let domain_filter = params.get("domain").and_then(|v| v.as_str());
            if let Some(domain) = domain_filter {
                let cookies = page.get_cookies().await.map_err(internal_err)?;
                let mut cleared = 0;
                for cookie in &cookies {
                    if cookie.domain.contains(domain) {
                        page.delete_cookie(&cookie.name, Some(&cookie.domain))
                            .await
                            .map_err(internal_err)?;
                        cleared += 1;
                    }
                }
                Ok(CallToolResult::success(vec![Content::text(format!(
                    "Cleared {cleared} cookie(s) for domain {domain}"
                ))]))
            } else {
                page.clear_cookies().await.map_err(internal_err)?;
                Ok(CallToolResult::success(vec![Content::text(
                    "All cookies cleared",
                )]))
            }
        }
        "save" => {
            let path = params
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| McpError::invalid_params("path is required for save", None))?;
            let cookies = page.get_cookies().await.map_err(internal_err)?;
            let domain_filter = params.get("domain").and_then(|v| v.as_str());
            let filtered: Vec<_> = if let Some(domain) = domain_filter {
                cookies
                    .into_iter()
                    .filter(|c| c.domain.contains(domain))
                    .collect()
            } else {
                cookies
            };
            let count = filtered.len();
            let json = serde_json::to_string(&filtered).map_err(|e| {
                McpError::internal_error(format!("Failed to serialize cookies: {e}"), None)
            })?;
            std::fs::write(path, &json).map_err(|e| {
                McpError::internal_error(format!("Failed to write {path}: {e}"), None)
            })?;
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Saved {count} cookie(s) to {path}"
            ))]))
        }
        "import" => {
            let browser_name = params
                .get("browser")
                .and_then(|v| v.as_str())
                .ok_or_else(|| McpError::invalid_params("browser is required for import", None))?;

            let browser = rayo_core::cookie_import::BrowserType::from_name(browser_name)
                .ok_or_else(|| {
                    McpError::invalid_params(
                        format!(
                            "Unknown browser: {browser_name}. \
                             Supported: chrome, arc, brave, edge, chromium"
                        ),
                        None,
                    )
                })?;

            let domain = params.get("domain").and_then(|v| v.as_str());
            let profile = params.get("profile").and_then(|v| v.as_str());

            let imported = rayo_core::cookie_import::import_cookies(browser, domain, profile)
                .map_err(internal_err)?;

            let count = imported.len();
            page.set_cookies(imported).await.map_err(internal_err)?;

            let domain_msg = domain
                .map(|d| format!(" for domain '{d}'"))
                .unwrap_or_default();
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Imported {count} cookie(s) from {browser_name}{domain_msg}"
            ))]))
        }
        "load" => {
            let path = params
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| McpError::invalid_params("path is required for load", None))?;
            let data = std::fs::read_to_string(path).map_err(|e| {
                McpError::internal_error(format!("Failed to read {path}: {e}"), None)
            })?;
            let cookie_infos: Vec<rayo_core::CookieInfo> = serde_json::from_str(&data)
                .map_err(|e| McpError::invalid_params(format!("Invalid cookie file: {e}"), None))?;
            let cookies: Vec<rayo_core::SetCookie> = cookie_infos
                .into_iter()
                .map(|c| rayo_core::SetCookie {
                    name: c.name,
                    value: c.value,
                    domain: Some(c.domain),
                    path: Some(c.path),
                    url: None,
                    secure: Some(c.secure),
                    http_only: Some(c.http_only),
                    same_site: c.same_site.as_deref().and_then(|s| match s {
                        "Strict" => Some(rayo_core::SameSite::Strict),
                        "Lax" => Some(rayo_core::SameSite::Lax),
                        "None" => Some(rayo_core::SameSite::None),
                        _ => None,
                    }),
                    expires: if c.expires > 0.0 {
                        Some(c.expires)
                    } else {
                        None
                    },
                })
                .collect();
            let count = cookies.len();
            page.set_cookies(cookies).await.map_err(internal_err)?;
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Loaded {count} cookie(s) from {path}"
            ))]))
        }
        _ => Err(McpError::invalid_params(
            format!("Unknown cookie action: {action}"),
            None,
        )),
    }
}

pub async fn handle_network(
    page: &RayoPage,
    network: &Arc<Mutex<NetworkInterceptor>>,
    params: &serde_json::Map<String, Value>,
) -> Result<CallToolResult, McpError> {
    let mode = params
        .get("mode")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("mode is required", None))?;

    let mut net = network.lock().await;

    match mode {
        "capture" => {
            net.start_capture();
            Ok(CallToolResult::success(vec![Content::text(
                "Network capture started. Use mode 'requests' to retrieve captured requests.",
            )]))
        }
        "requests" => {
            let url_pattern = params.get("url_pattern").and_then(|v| v.as_str());
            let requests = net.filtered_requests(url_pattern);
            let json = serde_json::to_string(&requests).unwrap_or_default();
            Ok(CallToolResult::success(vec![Content::text(format!(
                "{} request(s) captured\n{json}",
                requests.len()
            ))]))
        }
        "block" => {
            let url_pattern = params
                .get("url_pattern")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    McpError::invalid_params("url_pattern is required for block", None)
                })?;
            let resource_type = params
                .get("resource_type")
                .and_then(|v| v.as_str())
                .map(String::from);
            let need_fetch = !net.has_active_rules();
            net.add_block_rule(rayo_core::network::BlockRule {
                url_pattern: url_pattern.to_string(),
                resource_type,
            });
            // Enable Fetch interception on first block/mock rule
            if need_fetch {
                drop(net);
                page.enable_network_interception(Arc::clone(network))
                    .await
                    .map_err(internal_err)?;
            }
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Blocking requests matching: {url_pattern}"
            ))]))
        }
        "mock" => {
            let url_pattern = params
                .get("url_pattern")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    McpError::invalid_params("url_pattern is required for mock", None)
                })?;
            let response = params.get("response").ok_or_else(|| {
                McpError::invalid_params("response object is required for mock", None)
            })?;
            let status = response
                .get("status")
                .and_then(|v| v.as_u64())
                .unwrap_or(200) as u16;
            let body = response
                .get("body")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let headers = response
                .get("headers")
                .and_then(|v| v.as_object())
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|val| (k.clone(), val.to_string())))
                        .collect()
                })
                .unwrap_or_default();
            let resource_type = params
                .get("resource_type")
                .and_then(|v| v.as_str())
                .map(String::from);

            let need_fetch = !net.has_active_rules();
            net.add_mock_rule(rayo_core::network::MockRule {
                url_pattern: url_pattern.to_string(),
                status,
                body,
                headers,
                resource_type,
            });
            // Enable Fetch interception on first block/mock rule
            if need_fetch {
                drop(net);
                page.enable_network_interception(Arc::clone(network))
                    .await
                    .map_err(internal_err)?;
            }
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Mocking requests matching: {url_pattern} with status {status}"
            ))]))
        }
        "clear" => {
            net.clear_all();
            Ok(CallToolResult::success(vec![Content::text(
                "Network rules and captures cleared",
            )]))
        }
        _ => Err(McpError::invalid_params(
            format!("Unknown network mode: {mode}"),
            None,
        )),
    }
}

pub async fn handle_profile(
    profiler: &Arc<Profiler>,
    params: &serde_json::Map<String, Value>,
) -> Result<CallToolResult, McpError> {
    let format = params
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("ai_summary");
    let mut text = match format {
        "json" => profiler.export_json(),
        "markdown" => profiler.export_markdown(),
        "chrome_trace" => profiler.export_chrome_trace(),
        _ => profiler.export_ai_summary(),
    };

    // Append version info for human-readable formats
    if format != "json" && format != "chrome_trace" {
        text.push_str(&format!(
            "VERSION: rayo-mcp v{}\n",
            env!("CARGO_PKG_VERSION")
        ));
    }

    Ok(CallToolResult::success(vec![Content::text(text)]))
}
