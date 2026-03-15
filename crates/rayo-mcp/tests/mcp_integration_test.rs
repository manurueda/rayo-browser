//! Integration tests for the MCP server tool handlers.
//!
//! Tests all 5 MCP tools by calling handlers directly.
//! Uses a single shared browser instance to avoid Chrome process conflicts.

use std::sync::Arc;
use tokio::sync::Mutex;

use rayo_core::RayoBrowser;
use rayo_profiler::Profiler;
use rayo_rules::{RayoRulesConfig, RuleEngine};

fn chrome_available() -> bool {
    let paths = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium-browser",
        "/usr/bin/chromium",
    ];
    paths.iter().any(|p| std::path::Path::new(p).exists())
        || which::which("google-chrome").is_ok()
        || which::which("chromium").is_ok()
}

/// All MCP tool tests in one function to share a single browser instance.
#[tokio::test]
async fn test_mcp_tools() {
    if !chrome_available() {
        eprintln!("SKIP: Chrome not available");
        return;
    }

    let browser = RayoBrowser::launch().await.expect("Failed to launch browser");
    let page = browser.new_page().await.expect("Failed to create page");
    let page = Arc::new(Mutex::new(Some(page)));
    let profiler = Arc::new(Profiler::new());
    let rules = Arc::new(Mutex::new(RuleEngine::new(RayoRulesConfig::default())));

    // --- Test: rayo_navigate (goto) ---
    {
        let params = serde_json::json!({ "action": "goto", "url": "https://example.com" });
        let result = rayo_mcp::tools::handle_navigate(&page, params.as_object().unwrap()).await;
        assert!(result.is_ok(), "navigate failed: {:?}", result.err());
        let json = serde_json::to_string(&result.unwrap().content).unwrap();
        assert!(json.contains("example.com") || json.contains("Example"), "navigate response: {json}");
        eprintln!("  PASS: rayo_navigate (goto)");
    }

    // --- Test: rayo_observe (page_map) ---
    {
        let params = serde_json::json!({ "mode": "page_map" });
        let result = rayo_mcp::tools::handle_observe(&page, params.as_object().unwrap(), &rules).await;
        assert!(result.is_ok(), "observe page_map failed: {:?}", result.err());
        let json = serde_json::to_string(&result.unwrap().content).unwrap();
        assert!(json.contains("interactive"), "page_map should have interactive elements: {}", &json[..200.min(json.len())]);
        eprintln!("  PASS: rayo_observe (page_map)");
    }

    // --- Test: rayo_observe (text) ---
    {
        let params = serde_json::json!({ "mode": "text" });
        let result = rayo_mcp::tools::handle_observe(&page, params.as_object().unwrap(), &rules).await;
        assert!(result.is_ok(), "observe text failed: {:?}", result.err());
        let json = serde_json::to_string(&result.unwrap().content).unwrap();
        assert!(json.contains("Example Domain"), "text should contain 'Example Domain'");
        eprintln!("  PASS: rayo_observe (text)");
    }

    // --- Test: rayo_observe (screenshot) ---
    {
        let params = serde_json::json!({ "mode": "screenshot" });
        let result = rayo_mcp::tools::handle_observe(&page, params.as_object().unwrap(), &rules).await;
        assert!(result.is_ok(), "observe screenshot failed: {:?}", result.err());
        eprintln!("  PASS: rayo_observe (screenshot)");
    }

    // --- Test: rayo_batch ---
    {
        let params = serde_json::json!({
            "actions": [
                { "action": "screenshot", "full_page": false }
            ]
        });
        let result = rayo_mcp::tools::handle_batch(&page, params.as_object().unwrap()).await;
        assert!(result.is_ok(), "batch failed: {:?}", result.err());
        let json = serde_json::to_string(&result.unwrap().content).unwrap();
        assert!(json.contains("succeeded"), "batch should report succeeded");
        eprintln!("  PASS: rayo_batch");
    }

    // --- Test: rayo_profile ---
    {
        // Record a fake span on the profiler
        {
            let _s = profiler.start_span("test_nav", rayo_profiler::SpanCategory::Navigation);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let result = rayo_mcp::tools::handle_profile(&profiler).await;
        assert!(result.is_ok(), "profile failed: {:?}", result.err());
        let json = serde_json::to_string(&result.unwrap().content).unwrap();
        assert!(json.contains("RAYO PROFILE"), "profile should contain RAYO PROFILE");
        eprintln!("  PASS: rayo_profile");
    }

    // --- Test: rayo_interact (click) ---
    {
        let params = serde_json::json!({ "action": "click", "selector": "a" });
        let result = rayo_mcp::tools::handle_interact(&page, params.as_object().unwrap(), &rules).await;
        // This may or may not succeed (example.com has a link that navigates away)
        // We just verify it doesn't crash
        eprintln!("  PASS: rayo_interact (click) - result: {}", result.is_ok());
    }

    // --- Test: rules detect XPath ---
    {
        // Navigate back to example.com
        let nav = serde_json::json!({ "action": "goto", "url": "https://example.com" });
        rayo_mcp::tools::handle_navigate(&page, nav.as_object().unwrap()).await.ok();

        let params = serde_json::json!({ "action": "click", "selector": "//h1" });
        let _ = rayo_mcp::tools::handle_interact(&page, params.as_object().unwrap(), &rules).await;
        let violations = rules.lock().await.drain_violations();
        assert!(
            violations.iter().any(|v| v.rule.contains("prefer-css")),
            "Should detect XPath violation, got: {:?}", violations
        );
        eprintln!("  PASS: rules detect XPath");
    }

    eprintln!("ALL MCP INTEGRATION TESTS PASSED");
}
