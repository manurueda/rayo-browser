//! Integration tests for rayo-core browser operations.
//!
//! These tests require Chrome/Chromium to be installed.
//! They launch a real browser and test actual page interactions.
//!
//! Run with: cargo test --package rayo-core --test integration_test -- --test-threads=1

use rayo_core::RayoBrowser;
use rayo_core::batch::BatchAction;

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

/// Run all browser integration tests in a single test to share one browser instance.
/// This avoids Chrome process conflicts and ChannelSendError issues.
#[tokio::test]
async fn test_browser_operations() {
    if !chrome_available() {
        eprintln!("SKIP: Chrome not available");
        return;
    }

    let browser = RayoBrowser::launch().await.expect("Failed to launch browser");
    let page = browser.new_page().await.expect("Failed to create page");

    // --- Test: navigate and title ---
    page.goto("https://example.com").await.unwrap();
    let title = page.title().await.unwrap();
    assert!(title.contains("Example"), "Title should contain 'Example', got: {title}");
    let url = page.url().await.unwrap();
    assert!(url.contains("example.com"), "URL should contain example.com, got: {url}");
    eprintln!("  PASS: navigate_and_title");

    // --- Test: text content ---
    let text = page.text_content(None).await.unwrap();
    assert!(text.contains("Example Domain"), "Text should contain 'Example Domain'");
    eprintln!("  PASS: text_content");

    // --- Test: evaluate ---
    let result = page.evaluate("1 + 1").await.unwrap();
    assert_eq!(result, serde_json::json!(2), "1+1 should be 2");
    eprintln!("  PASS: evaluate");

    // --- Test: page map ---
    let map = page.page_map().await.unwrap();
    assert!(!map.url.is_empty(), "Page map URL should not be empty");
    assert!(!map.title.is_empty(), "Page map title should not be empty");
    assert!(!map.interactive.is_empty(), "Should have interactive elements");
    assert!(map.estimated_tokens() < 2000, "Page map should be < 2000 tokens, got: {}", map.estimated_tokens());
    eprintln!("  PASS: page_map ({} interactive elements, ~{} tokens)", map.interactive.len(), map.estimated_tokens());

    // --- Test: screenshot ---
    let b64 = page.screenshot(false).await.unwrap();
    assert!(b64.len() > 100, "Screenshot should be > 100 chars base64");
    eprintln!("  PASS: screenshot ({} bytes base64)", b64.len());

    // --- Test: batch execution ---
    let actions = vec![
        BatchAction::Screenshot { full_page: false },
    ];
    let batch_result = page.execute_batch(actions).await.unwrap();
    assert_eq!(batch_result.succeeded, 1);
    assert_eq!(batch_result.failed, 0);
    assert!(batch_result.all_succeeded());
    eprintln!("  PASS: batch_execution ({}ms)", batch_result.total_duration_ms as u64);

    // --- Test: wait for selector ---
    // h1 already exists on example.com
    page.wait_for_selector("h1", 5000).await.unwrap();
    eprintln!("  PASS: wait_for_selector");

    // --- Test: wait for selector timeout ---
    let timeout_result = page.wait_for_selector("#nonexistent-xyz", 500).await;
    assert!(timeout_result.is_err(), "Should timeout for nonexistent selector");
    eprintln!("  PASS: wait_for_selector_timeout");

    // --- Test: cookie set and get ---
    {
        use rayo_core::SetCookie;

        // Set a cookie
        let cookie = SetCookie {
            name: "test_session".into(),
            value: "abc123".into(),
            domain: None,
            path: None,
            url: None,
            secure: None,
            http_only: None,
            same_site: None,
            expires: None,
        };
        page.set_cookies(vec![cookie]).await.unwrap();

        // Read it back
        let cookies = page.get_cookies().await.unwrap();
        let found = cookies.iter().find(|c| c.name == "test_session");
        assert!(found.is_some(), "Should find test_session cookie");
        assert_eq!(found.unwrap().value, "abc123");
        eprintln!("  PASS: cookie_set_and_get");

        // Clear and verify
        page.clear_cookies().await.unwrap();
        let cookies = page.get_cookies().await.unwrap();
        let found = cookies.iter().find(|c| c.name == "test_session");
        assert!(found.is_none(), "Cookie should be cleared");
        eprintln!("  PASS: cookie_clear");
    }

    // --- Test: profiler records spans ---
    let spans = browser.profiler().spans();
    assert!(spans.len() >= 3, "Expected >= 3 spans, got: {}", spans.len());
    let summary = browser.profiler().export_ai_summary();
    assert!(summary.contains("RAYO PROFILE"), "Summary should contain RAYO PROFILE");
    eprintln!("  PASS: profiler ({} spans recorded)", spans.len());

    eprintln!("ALL INTEGRATION TESTS PASSED");
}
