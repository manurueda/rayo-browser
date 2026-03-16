//! Integration tests for the MCP server tool handlers.
//!
//! Tests all 7 MCP tools by calling handlers directly.
//! Uses a single shared browser instance to avoid Chrome process conflicts.
//! A local axum server serves test fixtures instead of hitting the network.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::sync::Mutex;

use rayo_core::RayoBrowser;
use rayo_core::network::NetworkInterceptor;
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

/// Start a local axum server serving static files from `tests/fixtures/`.
/// Returns the base URL (e.g. "http://127.0.0.1:<port>").
async fn start_fixture_server() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR"); // crates/rayo-mcp
    let fixtures_dir = std::path::PathBuf::from(manifest_dir)
        .join("../../tests/fixtures")
        .canonicalize()
        .expect("fixtures dir must exist");

    let serve_dir = tower_http::services::ServeDir::new(fixtures_dir);
    let app = axum::Router::new().fallback_service(serve_dir);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind random port");
    let addr: SocketAddr = listener.local_addr().unwrap();
    let base_url = format!("http://127.0.0.1:{}", addr.port());

    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    base_url
}

/// All MCP tool tests in one function to share a single browser instance.
#[tokio::test]
async fn test_mcp_tools() {
    if !chrome_available() {
        eprintln!("SKIP: Chrome not available");
        return;
    }

    let base_url = start_fixture_server().await;
    let browser = RayoBrowser::launch()
        .await
        .expect("Failed to launch browser");
    let page = browser.new_page().await.expect("Failed to create page");
    let profiler = Arc::new(Profiler::new());
    let rules = Arc::new(Mutex::new(RuleEngine::new(RayoRulesConfig::default())));
    let network = Arc::new(Mutex::new(NetworkInterceptor::new()));

    // --- Test: rayo_navigate (goto) ---
    {
        let params =
            serde_json::json!({ "action": "goto", "url": format!("{base_url}/index.html") });
        let result = rayo_mcp::tools::handle_navigate(&page, params.as_object().unwrap()).await;
        assert!(result.is_ok(), "navigate failed: {:?}", result.err());
        let json = serde_json::to_string(&result.unwrap().content).unwrap();
        assert!(
            json.contains("index.html") || json.contains("Rayo Test Page"),
            "navigate response: {json}"
        );
        eprintln!("  PASS: rayo_navigate (goto)");
    }

    // --- Test: rayo_observe (page_map) ---
    {
        let params = serde_json::json!({ "mode": "page_map" });
        let result =
            rayo_mcp::tools::handle_observe(&page, params.as_object().unwrap(), &rules).await;
        assert!(
            result.is_ok(),
            "observe page_map failed: {:?}",
            result.err()
        );
        let json = serde_json::to_string(&result.unwrap().content).unwrap();
        assert!(
            json.contains("interactive"),
            "page_map should have interactive elements: {}",
            &json[..200.min(json.len())]
        );
        eprintln!("  PASS: rayo_observe (page_map)");
    }

    // --- Test: rayo_observe (text) ---
    {
        let params = serde_json::json!({ "mode": "text" });
        let result =
            rayo_mcp::tools::handle_observe(&page, params.as_object().unwrap(), &rules).await;
        assert!(result.is_ok(), "observe text failed: {:?}", result.err());
        let json = serde_json::to_string(&result.unwrap().content).unwrap();
        assert!(
            json.contains("Test Page"),
            "text should contain 'Test Page'"
        );
        eprintln!("  PASS: rayo_observe (text)");
    }

    // --- Test: rayo_observe (screenshot) ---
    {
        let params = serde_json::json!({ "mode": "screenshot" });
        let result =
            rayo_mcp::tools::handle_observe(&page, params.as_object().unwrap(), &rules).await;
        assert!(
            result.is_ok(),
            "observe screenshot failed: {:?}",
            result.err()
        );
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

    // --- Test: rayo_cookie ---
    {
        let set_params = serde_json::json!({
            "action": "set",
            "cookies": [{ "name": "mcp_test", "value": "hello123" }]
        });
        let result = rayo_mcp::tools::handle_cookie(&page, set_params.as_object().unwrap()).await;
        assert!(result.is_ok(), "cookie set failed: {:?}", result.err());

        let get_params = serde_json::json!({ "action": "get" });
        let result = rayo_mcp::tools::handle_cookie(&page, get_params.as_object().unwrap()).await;
        assert!(result.is_ok(), "cookie get failed: {:?}", result.err());
        let json = serde_json::to_string(&result.unwrap().content).unwrap();
        assert!(
            json.contains("mcp_test"),
            "should find mcp_test cookie in response: {json}"
        );

        let clear_params = serde_json::json!({ "action": "clear" });
        let result = rayo_mcp::tools::handle_cookie(&page, clear_params.as_object().unwrap()).await;
        assert!(result.is_ok(), "cookie clear failed: {:?}", result.err());
        eprintln!("  PASS: rayo_cookie");
    }

    // --- Test: rayo_profile ---
    {
        // Record a fake span on the profiler
        {
            let _s = profiler.start_span("test_nav", rayo_profiler::SpanCategory::Navigation);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let params = serde_json::Map::new();
        let result = rayo_mcp::tools::handle_profile(&profiler, &params).await;
        assert!(result.is_ok(), "profile failed: {:?}", result.err());
        let json = serde_json::to_string(&result.unwrap().content).unwrap();
        assert!(
            json.contains("RAYO PROFILE"),
            "profile should contain RAYO PROFILE"
        );
        eprintln!("  PASS: rayo_profile");
    }

    // --- Test: rayo_interact (click) ---
    {
        // Navigate back to index first
        let nav = serde_json::json!({ "action": "goto", "url": format!("{base_url}/index.html") });
        rayo_mcp::tools::handle_navigate(&page, nav.as_object().unwrap())
            .await
            .ok();

        let params = serde_json::json!({ "action": "click", "selector": "#test-button" });
        let result =
            rayo_mcp::tools::handle_interact(&page, params.as_object().unwrap(), &rules).await;
        assert!(result.is_ok(), "interact click failed: {:?}", result.err());
        eprintln!("  PASS: rayo_interact (click)");
    }

    // --- Test: rayo_interact (type) on form ---
    {
        let nav = serde_json::json!({ "action": "goto", "url": format!("{base_url}/form.html") });
        rayo_mcp::tools::handle_navigate(&page, nav.as_object().unwrap())
            .await
            .ok();

        let params =
            serde_json::json!({ "action": "type", "selector": "#name", "value": "MCP Test" });
        let result =
            rayo_mcp::tools::handle_interact(&page, params.as_object().unwrap(), &rules).await;
        assert!(result.is_ok(), "interact type failed: {:?}", result.err());
        let json = serde_json::to_string(&result.unwrap().content).unwrap();
        assert!(
            json.contains("Typed"),
            "type response should contain 'Typed'"
        );
        eprintln!("  PASS: rayo_interact (type)");
    }

    // --- Test: rules detect XPath ---
    {
        let params = serde_json::json!({ "action": "click", "selector": "//h1" });
        let _ = rayo_mcp::tools::handle_interact(&page, params.as_object().unwrap(), &rules).await;
        let violations = rules.lock().await.drain_violations();
        assert!(
            violations.iter().any(|v| v.rule.contains("prefer-css")),
            "Should detect XPath violation, got: {:?}",
            violations
        );
        eprintln!("  PASS: rules detect XPath");
    }

    // --- Test: rayo_navigate (back/forward) ---
    {
        // Navigate to index, then form, then back
        let nav1 = serde_json::json!({ "action": "goto", "url": format!("{base_url}/index.html") });
        rayo_mcp::tools::handle_navigate(&page, nav1.as_object().unwrap())
            .await
            .unwrap();

        let nav2 = serde_json::json!({ "action": "goto", "url": format!("{base_url}/form.html") });
        rayo_mcp::tools::handle_navigate(&page, nav2.as_object().unwrap())
            .await
            .unwrap();

        let back = serde_json::json!({ "action": "back" });
        let result = rayo_mcp::tools::handle_navigate(&page, back.as_object().unwrap()).await;
        assert!(result.is_ok(), "navigate back failed: {:?}", result.err());
        let json = serde_json::to_string(&result.unwrap().content).unwrap();
        assert!(
            json.contains("index.html"),
            "After back should be on index.html, got: {json}"
        );

        let forward = serde_json::json!({ "action": "forward" });
        let result = rayo_mcp::tools::handle_navigate(&page, forward.as_object().unwrap()).await;
        assert!(
            result.is_ok(),
            "navigate forward failed: {:?}",
            result.err()
        );
        let json = serde_json::to_string(&result.unwrap().content).unwrap();
        assert!(
            json.contains("form.html"),
            "After forward should be on form.html, got: {json}"
        );
        eprintln!("  PASS: rayo_navigate (back/forward)");
    }

    // --- Test: rayo_network (capture, requests, clear) ---
    {
        // Start capture
        let capture_params = serde_json::json!({ "mode": "capture" });
        let result =
            rayo_mcp::tools::handle_network(&page, &network, capture_params.as_object().unwrap()).await;
        assert!(result.is_ok(), "network capture failed: {:?}", result.err());
        let json = serde_json::to_string(&result.unwrap().content).unwrap();
        assert!(json.contains("capture started"), "capture response: {json}");

        // Manually record a request to simulate a capture (since we don't have CDP fetch wired in tests)
        {
            let mut net = network.lock().await;
            net.record_request(rayo_core::network::CapturedRequest {
                url: format!("{base_url}/index.html"),
                method: "GET".into(),
                resource_type: "document".into(),
                status: Some(200),
                headers: vec![],
                timestamp_ms: 100.0,
                request_id: None,
            });
            net.record_request(rayo_core::network::CapturedRequest {
                url: format!("{base_url}/form.html"),
                method: "GET".into(),
                resource_type: "document".into(),
                status: Some(200),
                headers: vec![],
                timestamp_ms: 200.0,
                request_id: None,
            });
        }

        // Get requests (unfiltered)
        let requests_params = serde_json::json!({ "mode": "requests" });
        let result =
            rayo_mcp::tools::handle_network(&page, &network, requests_params.as_object().unwrap()).await;
        assert!(
            result.is_ok(),
            "network requests failed: {:?}",
            result.err()
        );
        let json = serde_json::to_string(&result.unwrap().content).unwrap();
        assert!(
            json.contains("2 request(s) captured"),
            "should see 2 requests: {json}"
        );

        // Get requests (filtered)
        let filtered_params = serde_json::json!({ "mode": "requests", "url_pattern": "*form*" });
        let result =
            rayo_mcp::tools::handle_network(&page, &network, filtered_params.as_object().unwrap()).await;
        assert!(
            result.is_ok(),
            "network filtered requests failed: {:?}",
            result.err()
        );
        let json = serde_json::to_string(&result.unwrap().content).unwrap();
        assert!(
            json.contains("1 request(s) captured"),
            "should see 1 filtered request: {json}"
        );

        // Clear
        let clear_params = serde_json::json!({ "mode": "clear" });
        let result =
            rayo_mcp::tools::handle_network(&page, &network, clear_params.as_object().unwrap()).await;
        assert!(result.is_ok(), "network clear failed: {:?}", result.err());
        let json = serde_json::to_string(&result.unwrap().content).unwrap();
        assert!(json.contains("cleared"), "clear response: {json}");

        // Verify capture was cleared
        let requests_params = serde_json::json!({ "mode": "requests" });
        let result =
            rayo_mcp::tools::handle_network(&page, &network, requests_params.as_object().unwrap()).await;
        assert!(
            result.is_ok(),
            "network requests after clear failed: {:?}",
            result.err()
        );
        let json = serde_json::to_string(&result.unwrap().content).unwrap();
        assert!(
            json.contains("0 request(s)"),
            "should see 0 requests after clear: {json}"
        );
        eprintln!("  PASS: rayo_network (capture, requests, filter, clear)");
    }

    // --- Test: multi-tab via handlers ---
    {
        // Create two pages and navigate each to a different URL
        let page1 = browser.new_page().await.expect("Failed to create page1");
        let page2 = browser.new_page().await.expect("Failed to create page2");

        let nav1 = serde_json::json!({ "action": "goto", "url": format!("{base_url}/index.html") });
        let result = rayo_mcp::tools::handle_navigate(&page1, nav1.as_object().unwrap()).await;
        assert!(result.is_ok(), "navigate page1 failed: {:?}", result.err());

        let nav2 = serde_json::json!({ "action": "goto", "url": format!("{base_url}/form.html") });
        let result = rayo_mcp::tools::handle_navigate(&page2, nav2.as_object().unwrap()).await;
        assert!(result.is_ok(), "navigate page2 failed: {:?}", result.err());

        // Observe each tab independently
        let params = serde_json::json!({ "mode": "text" });
        let result1 =
            rayo_mcp::tools::handle_observe(&page1, params.as_object().unwrap(), &rules).await;
        let result2 =
            rayo_mcp::tools::handle_observe(&page2, params.as_object().unwrap(), &rules).await;
        assert!(result1.is_ok(), "observe page1 failed");
        assert!(result2.is_ok(), "observe page2 failed");

        let text1 = serde_json::to_string(&result1.unwrap().content).unwrap();
        let text2 = serde_json::to_string(&result2.unwrap().content).unwrap();
        assert!(
            text1.contains("Test Page"),
            "page1 should show index content: {text1}"
        );
        assert!(
            text2.contains("Test Form"),
            "page2 should show form content: {text2}"
        );
        eprintln!("  PASS: multi_tab via handlers");
    }

    // --- Test: batch with mixed actions (goto + screenshot) via handler ---
    {
        let params = serde_json::json!({
            "actions": [
                { "action": "goto", "url": format!("{base_url}/index.html") },
                { "action": "screenshot", "full_page": false }
            ]
        });
        let result = rayo_mcp::tools::handle_batch(&page, params.as_object().unwrap()).await;
        assert!(result.is_ok(), "batch mixed failed: {:?}", result.err());
        let json = serde_json::to_string(&result.unwrap().content).unwrap();
        // The JSON is pretty-printed inside a Content::text, then serialized again
        // so check for escaped versions
        assert!(
            json.contains("succeeded") && json.contains("failed"),
            "batch should report results: {}",
            &json[..500.min(json.len())]
        );
        eprintln!("  PASS: rayo_batch (goto + screenshot)");
    }

    eprintln!("ALL MCP INTEGRATION TESTS PASSED");
}
