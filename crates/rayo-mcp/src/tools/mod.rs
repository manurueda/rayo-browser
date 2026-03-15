//! MCP tool handlers.
//!
//! 5 tools, ~1,500 tokens total tool description.
//! Each handler takes the page lock and params, returns CallToolResult.

pub mod batch;
pub mod cookie;
pub use cookie::handle_cookie;
pub mod interact;
pub mod navigate;
pub mod observe;
pub mod profile;

use std::sync::Arc;

use rmcp::model::{CallToolResult, Content};
use rmcp::Error as McpError;
use rayo_core::RayoPage;
use rayo_profiler::Profiler;
use rayo_rules::RuleEngine;
use serde_json::Value;
use tokio::sync::Mutex;

pub async fn handle_navigate(
    page: &Arc<Mutex<Option<RayoPage>>>,
    params: &serde_json::Map<String, Value>,
) -> Result<CallToolResult, McpError> {
    let page_guard = page.lock().await;
    let page = page_guard
        .as_ref()
        .ok_or_else(|| McpError::internal_error("No page available", None))?;

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
            page.goto(url)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            let title = page.title().await.unwrap_or_default();
            let current_url = page.url().await.unwrap_or_default();
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Navigated to {current_url}\nTitle: {title}"
            ))]))
        }
        "reload" => {
            page.reload()
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            Ok(CallToolResult::success(vec![Content::text("Page reloaded")]))
        }
        "back" => {
            page.evaluate("history.back()")
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            Ok(CallToolResult::success(vec![Content::text("Navigated back")]))
        }
        "forward" => {
            page.evaluate("history.forward()")
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            Ok(CallToolResult::success(vec![Content::text(
                "Navigated forward",
            )]))
        }
        _ => Err(McpError::invalid_params(
            format!("Unknown navigate action: {action}"),
            None,
        )),
    }
}

pub async fn handle_observe(
    page: &Arc<Mutex<Option<RayoPage>>>,
    params: &serde_json::Map<String, Value>,
    rules: &Arc<Mutex<RuleEngine>>,
) -> Result<CallToolResult, McpError> {
    let page_guard = page.lock().await;
    let page = page_guard
        .as_ref()
        .ok_or_else(|| McpError::internal_error("No page available", None))?;

    let mode = params
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("page_map");

    match mode {
        "page_map" => {
            let map = page
                .page_map()
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            let json = serde_json::to_string_pretty(&map).unwrap_or_default();
            Ok(CallToolResult::success(vec![Content::text(json)]))
        }
        "text" => {
            let selector = params.get("selector").and_then(|v| v.as_str());
            let text = page
                .text_content(selector)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            Ok(CallToolResult::success(vec![Content::text(text)]))
        }
        "screenshot" => {
            // Check rate limit
            rules.lock().await.check_screenshot();

            let full_page = params
                .get("full_page")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let b64 = page
                .screenshot(full_page)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            Ok(CallToolResult::success(vec![Content::image(
                b64, "image/png",
            )]))
        }
        _ => Err(McpError::invalid_params(
            format!("Unknown observe mode: {mode}"),
            None,
        )),
    }
}

pub async fn handle_interact(
    page: &Arc<Mutex<Option<RayoPage>>>,
    params: &serde_json::Map<String, Value>,
    rules: &Arc<Mutex<RuleEngine>>,
) -> Result<CallToolResult, McpError> {
    let page_guard = page.lock().await;
    let page = page_guard
        .as_ref()
        .ok_or_else(|| McpError::internal_error("No page available", None))?;

    let action = params
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("action is required", None))?;

    // Check selector rules
    if let Some(sel) = params.get("selector").and_then(|v| v.as_str()) {
        rules.lock().await.check_selector(sel);
    }

    let id = params.get("id").and_then(|v| v.as_u64()).map(|v| v as usize);
    let selector = params.get("selector").and_then(|v| v.as_str());
    let value = params.get("value").and_then(|v| v.as_str());

    match action {
        "click" => {
            page.click(selector, id)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            Ok(CallToolResult::success(vec![Content::text("Clicked")]))
        }
        "type" => {
            let text = value
                .ok_or_else(|| McpError::invalid_params("value is required for type", None))?;
            page.type_text(selector, id, text, true)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Typed: {text}"
            ))]))
        }
        "select" => {
            let val = value
                .ok_or_else(|| McpError::invalid_params("value is required for select", None))?;
            page.select_option(selector, id, val)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Selected: {val}"
            ))]))
        }
        "scroll" => {
            if let Some(sel) = selector {
                let js = format!(
                    "document.querySelector({}).scrollIntoView({{block:'center'}})",
                    serde_json::to_string(sel).unwrap()
                );
                page.evaluate(&js)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            } else if let Some(element_id) = id {
                page.click(None, Some(element_id))
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            }
            Ok(CallToolResult::success(vec![Content::text("Scrolled")]))
        }
        _ => Err(McpError::invalid_params(
            format!("Unknown interact action: {action}"),
            None,
        )),
    }
}

pub async fn handle_batch(
    page: &Arc<Mutex<Option<RayoPage>>>,
    params: &serde_json::Map<String, Value>,
) -> Result<CallToolResult, McpError> {
    let page_guard = page.lock().await;
    let page = page_guard
        .as_ref()
        .ok_or_else(|| McpError::internal_error("No page available", None))?;

    let actions_value = params
        .get("actions")
        .ok_or_else(|| McpError::invalid_params("actions array is required", None))?;

    let actions: Vec<rayo_core::batch::BatchAction> = serde_json::from_value(actions_value.clone())
        .map_err(|e| McpError::invalid_params(format!("Invalid actions: {e}"), None))?;

    let result = page
        .execute_batch(actions)
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    let json = serde_json::to_string_pretty(&result).unwrap_or_default();
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

pub async fn handle_profile(profiler: &Arc<Profiler>) -> Result<CallToolResult, McpError> {
    let summary = profiler.export_ai_summary();
    Ok(CallToolResult::success(vec![Content::text(summary)]))
}
