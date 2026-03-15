//! Cookie management tool handler.

use std::sync::Arc;

use rayo_core::{RayoPage, SameSite, SetCookie};
use rmcp::Error as McpError;
use rmcp::model::{CallToolResult, Content};
use serde_json::Value;
use tokio::sync::Mutex;

pub async fn handle_cookie(
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

                cookies.push(SetCookie {
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
                            "Strict" => Some(SameSite::Strict),
                            "Lax" => Some(SameSite::Lax),
                            "None" => Some(SameSite::None),
                            _ => None,
                        },
                    ),
                    expires: entry.get("expires").and_then(|v| v.as_f64()),
                });
            }

            let count = cookies.len();
            page.set_cookies(cookies)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

            Ok(CallToolResult::success(vec![Content::text(format!(
                "Set {count} cookie(s)"
            ))]))
        }
        "get" => {
            let cookies = page
                .get_cookies()
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

            let domain_filter = params.get("domain").and_then(|v| v.as_str());

            let filtered: Vec<_> = if let Some(domain) = domain_filter {
                cookies
                    .into_iter()
                    .filter(|c| c.domain.contains(domain))
                    .collect()
            } else {
                cookies
            };

            let json = serde_json::to_string_pretty(&filtered).unwrap_or_default();
            Ok(CallToolResult::success(vec![Content::text(json)]))
        }
        "clear" => {
            let domain_filter = params.get("domain").and_then(|v| v.as_str());

            if let Some(domain) = domain_filter {
                let cookies = page
                    .get_cookies()
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                let mut cleared = 0;
                for cookie in &cookies {
                    if cookie.domain.contains(domain) {
                        page.delete_cookie(&cookie.name, Some(&cookie.domain))
                            .await
                            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                        cleared += 1;
                    }
                }
                Ok(CallToolResult::success(vec![Content::text(format!(
                    "Cleared {cleared} cookie(s) for domain {domain}"
                ))]))
            } else {
                page.clear_cookies()
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(
                    "All cookies cleared",
                )]))
            }
        }
        _ => Err(McpError::invalid_params(
            format!("Unknown cookie action: {action}"),
            None,
        )),
    }
}
