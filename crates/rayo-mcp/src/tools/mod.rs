//! MCP tool handlers.
//!
//! 9 tools, ~2,500 tokens total tool description.
//! Handlers receive a resolved &RayoPage — tab resolution is done by the server.

use std::sync::Arc;

use rayo_core::RayoPage;
use rayo_core::network::NetworkInterceptor;
use rayo_profiler::Profiler;
use rayo_rules::RuleEngine;
use rmcp::Error as McpError;
use rmcp::model::{CallToolResult, Content};
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::error_collector::ErrorCollector;

/// Helper to convert RayoError or similar into McpError.
fn internal_err(e: impl std::fmt::Display) -> McpError {
    McpError::internal_error(e.to_string(), None)
}

pub async fn handle_navigate(
    page: &RayoPage,
    params: &serde_json::Map<String, Value>,
    llm_checker: Option<&rayo_core::auth::LlmAuthChecker>,
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

            let nav = page
                .goto_with_auto_auth(url, llm_checker)
                .await
                .map_err(internal_err)?;

            if wait_until == "networkidle" {
                page.wait_for_network_idle(500, 5000)
                    .await
                    .map_err(internal_err)?;
            }

            let map = if wait_until == "networkidle" {
                page.page_map(None).await.map_err(internal_err)?
            } else {
                nav.map.clone()
            };
            let json = serde_json::to_string(&map).unwrap_or_default();

            let mut status = format!("Navigated to {}\nTitle: {}", map.url, map.title);

            if nav.redirected {
                status.push_str(&format!("\n⚠ Redirected from {}", nav.requested_url));
            }

            match nav.auto_auth {
                rayo_core::AutoAuthStatus::Succeeded => {
                    status.push_str("\n✓ Auto-auth: imported cookies and retried successfully");
                }
                rayo_core::AutoAuthStatus::Failed => {
                    status.push_str(
                        "\n⚠ Auto-auth: auth wall detected but cookie import failed — you may be seeing a login page",
                    );
                }
                rayo_core::AutoAuthStatus::NotNeeded => {}
            }

            if wait_until != "load" {
                status.push_str(&format!(" (waited for {wait_until})"));
            }

            let content = vec![
                Content::text(status),
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
        "inspect" => {
            let selector = params.get("selector").and_then(|v| v.as_str());
            let id = params
                .get("id")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);

            if selector.is_none() && id.is_none() {
                return Err(McpError::invalid_params(
                    "inspect requires 'id' (page_map element ID) or 'selector' (CSS selector). Use selector for non-interactive elements.",
                    None,
                ));
            }

            let properties = params
                .get("properties")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                });
            let all = params.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
            let compact = params
                .get("compact")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let diff = params
                .get("diff")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let expect = params.get("expect").and_then(|v| v.as_object()).map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|val| (k.clone(), val.to_string())))
                    .collect()
            });

            let options = rayo_core::inspect::InspectOptions {
                properties,
                all,
                compact,
                diff,
                expect,
            };

            // Support multi-element via ids array
            let ids: Option<Vec<usize>> = params.get("ids").and_then(|v| v.as_array()).map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as usize))
                    .collect()
            });

            if let Some(ids) = ids {
                let mut results = Vec::new();
                for eid in ids {
                    match page.inspect_element(None, Some(eid), &options).await {
                        Ok(r) => results.push(serde_json::to_value(&r).unwrap_or_default()),
                        Err(e) => {
                            results.push(serde_json::json!({ "error": e.to_string(), "id": eid }))
                        }
                    }
                }
                let json = serde_json::to_string(&results).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(json)]))
            } else {
                let result = page
                    .inspect_element(selector, id, &options)
                    .await
                    .map_err(internal_err)?;
                let json = serde_json::to_string(&result).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
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
                    same_site: entry
                        .get("sameSite")
                        .and_then(|v| v.as_str())
                        .and_then(rayo_core::SameSite::parse),
                    expires: entry.get("expires").and_then(|v| v.as_f64()),
                });
            }

            let result = page.set_cookies(cookies).await.map_err(internal_err)?;
            let mut msg = format!("Set {} cookie(s)", result.set);
            if !result.failed.is_empty() {
                msg.push_str(&format!(
                    " ({} failed: {:?})",
                    result.failed.len(),
                    result.failed
                ));
            }
            Ok(CallToolResult::success(vec![Content::text(msg)]))
        }
        "get" => {
            let cookies = page.get_cookies().await.map_err(internal_err)?;
            let domain_filter = params.get("domain").and_then(|v| v.as_str());
            let filtered: Vec<_> = if let Some(domain) = domain_filter {
                cookies
                    .into_iter()
                    .filter(|c| rayo_core::matches_domain(&c.domain, domain))
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
                    if rayo_core::matches_domain(&cookie.domain, domain) {
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
                    .filter(|c| rayo_core::matches_domain(&c.domain, domain))
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

            let import = rayo_core::cookie_import::import_cookies(browser, domain, profile)
                .map_err(internal_err)?;

            if import.cookies.is_empty() {
                let mut msg = format!(
                    "No cookies found in {browser_name} (profile: '{}')",
                    import.profile_used,
                );
                if let Some(d) = domain {
                    msg.push_str(&format!(" for domain '{d}'"));
                }
                if !import.decrypt_failed.is_empty() {
                    msg.push_str(&format!(
                        ". {} cookie(s) failed decryption: {:?}",
                        import.decrypt_failed.len(),
                        import.decrypt_failed,
                    ));
                }
                let other_profiles: Vec<_> = import
                    .available_profiles
                    .iter()
                    .filter(|p| p.as_str() != import.profile_used)
                    .collect();
                if !other_profiles.is_empty() {
                    msg.push_str(&format!(". Other profiles available: {other_profiles:?}",));
                }
                return Ok(CallToolResult::success(vec![Content::text(msg)]));
            }

            let set_result = page
                .set_cookies(import.cookies)
                .await
                .map_err(internal_err)?;

            let domain_msg = domain
                .map(|d| format!(" for domain '{d}'"))
                .unwrap_or_default();

            let mut msg = format!(
                "Imported {} cookie(s) from {browser_name} (profile: '{}'){domain_msg}",
                set_result.set, import.profile_used,
            );
            if !set_result.failed.is_empty() {
                msg.push_str(&format!(
                    " ({} rejected by Chrome: {:?})",
                    set_result.failed.len(),
                    set_result.failed,
                ));
            }
            if !import.decrypt_failed.is_empty() {
                msg.push_str(&format!(
                    " ({} failed decryption)",
                    import.decrypt_failed.len(),
                ));
            }
            Ok(CallToolResult::success(vec![Content::text(msg)]))
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
                    same_site: c.same_site.as_deref().and_then(rayo_core::SameSite::parse),
                    expires: if c.expires > 0.0 {
                        Some(c.expires)
                    } else {
                        None
                    },
                })
                .collect();
            let result = page.set_cookies(cookies).await.map_err(internal_err)?;
            let mut msg = format!("Loaded {} cookie(s) from {path}", result.set);
            if !result.failed.is_empty() {
                msg.push_str(&format!(" ({} failed)", result.failed.len()));
            }
            Ok(CallToolResult::success(vec![Content::text(msg)]))
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

pub async fn handle_visual(
    page: &RayoPage,
    params: &serde_json::Map<String, Value>,
    profiler: &Arc<Profiler>,
) -> Result<CallToolResult, McpError> {
    let action = params
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("action is required", None))?;

    let baselines_dir = std::env::current_dir()
        .unwrap_or_default()
        .join(".rayo")
        .join("baselines");
    let manager = rayo_visual::BaselineManager::new(baselines_dir);

    match action {
        "capture" => {
            let _span =
                profiler.start_span("visual_capture", rayo_profiler::SpanCategory::Screenshot);

            let name = params
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| McpError::invalid_params("name is required for capture", None))?;
            let full_page = params
                .get("full_page")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let selector = params.get("selector").and_then(|v| v.as_str());

            // Freeze animations for deterministic screenshots
            page.freeze_animations().await.map_err(internal_err)?;

            let png_bytes = if let Some(sel) = selector {
                page.screenshot_element(sel).await.map_err(internal_err)?
            } else {
                page.screenshot_png(full_page).await.map_err(internal_err)?
            };

            page.unfreeze_animations().await.map_err(internal_err)?;

            // Get dimensions from the captured PNG
            let img = image::load_from_memory(&png_bytes).map_err(|e| {
                McpError::internal_error(format!("Failed to decode screenshot: {e}"), None)
            })?;
            let (w, h) = image::GenericImageView::dimensions(&img);

            manager.save(name, &png_bytes, w, h).map_err(internal_err)?;

            let result = serde_json::json!({
                "action": "capture",
                "name": name,
                "new_baseline": true,
                "dimensions": [w, h],
            });
            Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string(&result).unwrap_or_default(),
            )]))
        }
        "compare" => {
            let _span =
                profiler.start_span("visual_compare", rayo_profiler::SpanCategory::Screenshot);

            let name = params
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| McpError::invalid_params("name is required for compare", None))?;
            let threshold = params
                .get("threshold")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.01);
            let full_page = params
                .get("full_page")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let selector = params.get("selector").and_then(|v| v.as_str());

            // Freeze animations for deterministic screenshots
            page.freeze_animations().await.map_err(internal_err)?;

            let current_png = if let Some(sel) = selector {
                page.screenshot_element(sel).await.map_err(internal_err)?
            } else {
                page.screenshot_png(full_page).await.map_err(internal_err)?
            };

            page.unfreeze_animations().await.map_err(internal_err)?;

            // If baseline doesn't exist, auto-create it
            if !manager.exists(name) {
                let img = image::load_from_memory(&current_png).map_err(|e| {
                    McpError::internal_error(format!("Failed to decode screenshot: {e}"), None)
                })?;
                let (w, h) = image::GenericImageView::dimensions(&img);
                manager
                    .save(name, &current_png, w, h)
                    .map_err(internal_err)?;

                let result = serde_json::json!({
                    "action": "compare",
                    "name": name,
                    "new_baseline": true,
                    "dimensions": [w, h],
                });
                return Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string(&result).unwrap_or_default(),
                )]));
            }

            let baseline_png = manager.load(name).map_err(internal_err)?;

            let options = rayo_visual::DiffOptions {
                threshold,
                ..Default::default()
            };

            let report = rayo_visual::compare(&baseline_png, &current_png, &options)
                .map_err(internal_err)?;

            let report_json = serde_json::to_string(&report).unwrap_or_default();
            Ok(CallToolResult::success(vec![Content::text(report_json)]))
        }
        "baseline" => {
            let _span =
                profiler.start_span("visual_baseline", rayo_profiler::SpanCategory::Screenshot);

            let mode = params.get("mode").and_then(|v| v.as_str()).ok_or_else(|| {
                McpError::invalid_params("mode is required for baseline action", None)
            })?;

            match mode {
                "list" => {
                    let list = manager.list().map_err(internal_err)?;
                    let json = serde_json::to_string(&list).unwrap_or_default();
                    Ok(CallToolResult::success(vec![Content::text(json)]))
                }
                "delete" => {
                    let name = params.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
                        McpError::invalid_params("name is required for delete", None)
                    })?;
                    manager.delete(name).map_err(internal_err)?;
                    Ok(CallToolResult::success(vec![Content::text(format!(
                        "Deleted baseline: {name}"
                    ))]))
                }
                "update" => {
                    let name = params.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
                        McpError::invalid_params("name is required for update", None)
                    })?;
                    let full_page = params
                        .get("full_page")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let selector = params.get("selector").and_then(|v| v.as_str());

                    page.freeze_animations().await.map_err(internal_err)?;

                    let png_bytes = if let Some(sel) = selector {
                        page.screenshot_element(sel).await.map_err(internal_err)?
                    } else {
                        page.screenshot_png(full_page).await.map_err(internal_err)?
                    };

                    page.unfreeze_animations().await.map_err(internal_err)?;

                    let img = image::load_from_memory(&png_bytes).map_err(|e| {
                        McpError::internal_error(format!("Failed to decode screenshot: {e}"), None)
                    })?;
                    let (w, h) = image::GenericImageView::dimensions(&img);

                    manager.save(name, &png_bytes, w, h).map_err(internal_err)?;

                    let result = serde_json::json!({
                        "action": "baseline",
                        "mode": "update",
                        "name": name,
                        "new_baseline": true,
                        "dimensions": [w, h],
                    });
                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string(&result).unwrap_or_default(),
                    )]))
                }
                _ => Err(McpError::invalid_params(
                    format!("Unknown baseline mode: {mode}. Use list, delete, or update."),
                    None,
                )),
            }
        }
        _ => Err(McpError::invalid_params(
            format!("Unknown visual action: {action}. Use capture, compare, or baseline."),
            None,
        )),
    }
}

pub async fn handle_report(
    errors: &Mutex<ErrorCollector>,
    params: &serde_json::Map<String, Value>,
) -> Result<CallToolResult, McpError> {
    let action = params
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("get");

    match action {
        "get" => {
            let last_n = params.get("last_n").and_then(|v| v.as_u64());
            let collector = errors.lock().await;
            let all_errors = collector.report();

            if all_errors.is_empty() {
                return Ok(CallToolResult::success(vec![Content::text(
                    "No errors recorded.",
                )]));
            }

            let errors_slice: Vec<_> = if let Some(n) = last_n {
                all_errors.iter().rev().take(n as usize).rev().collect()
            } else {
                all_errors.iter().collect()
            };

            let report = json!({
                "error_count": errors_slice.len(),
                "rayo_version": collector.version(),
                "errors": errors_slice,
                "hint": "File an issue at https://github.com/manurueda/rayo-browser/issues/new with this report"
            });
            Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&report).unwrap_or_default(),
            )]))
        }
        "clear" => {
            errors.lock().await.clear();
            Ok(CallToolResult::success(vec![Content::text(
                "Error log cleared.",
            )]))
        }
        _ => Err(McpError::invalid_params(
            format!("Unknown report action: {action}"),
            None,
        )),
    }
}
