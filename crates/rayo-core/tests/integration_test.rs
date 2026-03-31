//! Integration tests for rayo-core browser operations.
//!
//! These tests require Chrome/Chromium to be installed.
//! They launch a real browser and test actual page interactions.
//! A local axum server serves test fixtures instead of hitting the network.
//!
//! Run with: cargo test --package rayo-core --test integration_test -- --test-threads=1

use std::net::SocketAddr;

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

/// Start a local axum server serving static files from `tests/fixtures/`.
/// Returns the base URL (e.g. "http://127.0.0.1:<port>").
async fn start_fixture_server() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR"); // crates/rayo-core
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

/// Run all browser integration tests in a single test to share one browser instance.
/// This avoids Chrome process conflicts and ChannelSendError issues.
#[tokio::test]
async fn test_browser_operations() {
    if !chrome_available() {
        eprintln!("SKIP: Chrome not available");
        return;
    }

    let base_url = start_fixture_server().await;
    let browser = RayoBrowser::launch()
        .await
        .expect("Failed to launch browser");
    let page = browser.new_page().await.expect("Failed to create page");

    // --- Test: navigate and title ---
    page.goto(&format!("{base_url}/index.html")).await.unwrap();
    let title = page.title().await.unwrap();
    assert!(
        title.contains("Rayo Test Page"),
        "Title should contain 'Rayo Test Page', got: {title}"
    );
    let url = page.url().await.unwrap();
    assert!(
        url.contains("/index.html"),
        "URL should contain /index.html, got: {url}"
    );
    eprintln!("  PASS: navigate_and_title");

    // --- Test: text content ---
    let text = page.text_content(None, 50).await.unwrap();
    assert!(
        text.contains("Test Page"),
        "Text should contain 'Test Page'"
    );
    eprintln!("  PASS: text_content");

    // --- Test: evaluate ---
    let result = page.evaluate("1 + 1").await.unwrap();
    assert_eq!(result, serde_json::json!(2), "1+1 should be 2");
    eprintln!("  PASS: evaluate");

    // --- Test: page map ---
    let map = page.page_map(None).await.unwrap();
    assert!(!map.url.is_empty(), "Page map URL should not be empty");
    assert!(!map.title.is_empty(), "Page map title should not be empty");
    assert!(
        !map.interactive.is_empty(),
        "Should have interactive elements"
    );
    assert!(
        map.estimated_tokens() < 2000,
        "Page map should be < 2000 tokens, got: {}",
        map.estimated_tokens()
    );
    eprintln!(
        "  PASS: page_map ({} interactive elements, ~{} tokens)",
        map.interactive.len(),
        map.estimated_tokens()
    );

    // --- Test: screenshot ---
    let b64 = page.screenshot(false).await.unwrap();
    assert!(b64.len() > 100, "Screenshot should be > 100 chars base64");
    eprintln!("  PASS: screenshot ({} bytes base64)", b64.len());

    // --- Test: batch execution ---
    let actions = vec![BatchAction::Screenshot { full_page: false }];
    let batch_result = page.execute_batch(actions, false).await.unwrap();
    assert_eq!(batch_result.succeeded, 1);
    assert_eq!(batch_result.failed, 0);
    assert!(batch_result.all_succeeded());
    eprintln!(
        "  PASS: batch_execution ({}ms)",
        batch_result.total_duration_ms as u64
    );

    // --- Test: wait for selector ---
    // h1 already exists on index.html
    page.wait_for_selector("h1", 5000, false).await.unwrap();
    eprintln!("  PASS: wait_for_selector");

    // --- Test: wait for selector timeout ---
    let timeout_result = page.wait_for_selector("#nonexistent-xyz", 500, false).await;
    assert!(
        timeout_result.is_err(),
        "Should timeout for nonexistent selector"
    );
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

    // --- Test: form fill ---
    {
        page.goto(&format!("{base_url}/form.html")).await.unwrap();
        let title = page.title().await.unwrap();
        assert!(
            title.contains("Test Form"),
            "Should be on form page, got: {title}"
        );

        // Get page map to verify form elements are detected
        let map = page.page_map(None).await.unwrap();
        assert!(
            map.interactive
                .iter()
                .any(|e| e.selector.contains("#name") || e.selector.contains("input")),
            "Page map should include the name input"
        );

        // Type into the name field
        page.type_text(Some("#name"), None, "Rayo Test User", true)
            .await
            .unwrap();

        // Verify the value was set
        let value = page
            .evaluate("document.querySelector('#name').value")
            .await
            .unwrap();
        assert_eq!(
            value.as_str().unwrap_or(""),
            "Rayo Test User",
            "Name field should have typed value"
        );
        eprintln!("  PASS: form_fill");
    }

    // --- Test: back/forward navigation ---
    {
        // Navigate to index.html first, then to form.html, then go back
        page.goto(&format!("{base_url}/index.html")).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        page.goto(&format!("{base_url}/form.html")).await.unwrap();
        let url = page.url().await.unwrap();
        assert!(
            url.contains("/form.html"),
            "Should be on form.html, got: {url}"
        );

        // Go back via history
        page.evaluate("history.back()").await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let url = page.url().await.unwrap();
        assert!(
            url.contains("/index.html"),
            "After back, should be on index.html, got: {url}"
        );

        // Go forward
        page.evaluate("history.forward()").await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let url = page.url().await.unwrap();
        assert!(
            url.contains("/form.html"),
            "After forward, should be on form.html, got: {url}"
        );
        eprintln!("  PASS: back_forward_navigation");
    }

    // --- Test: multi-tab ---
    {
        let page1 = browser.new_page().await.expect("Failed to create page1");
        let page2 = browser.new_page().await.expect("Failed to create page2");

        page1.goto(&format!("{base_url}/index.html")).await.unwrap();
        page2.goto(&format!("{base_url}/form.html")).await.unwrap();

        let url1 = page1.url().await.unwrap();
        let url2 = page2.url().await.unwrap();

        assert!(
            url1.contains("/index.html"),
            "Page1 should be on index.html, got: {url1}"
        );
        assert!(
            url2.contains("/form.html"),
            "Page2 should be on form.html, got: {url2}"
        );

        // Verify they are independent — page2 title should be form page
        let title2 = page2.title().await.unwrap();
        assert!(
            title2.contains("Test Form"),
            "Page2 title should be form, got: {title2}"
        );
        eprintln!("  PASS: multi_tab");
    }

    // --- Test: batch with mixed actions (goto + screenshot) ---
    {
        let actions = vec![
            BatchAction::Goto {
                url: format!("{base_url}/index.html"),
            },
            BatchAction::Screenshot { full_page: false },
        ];
        let batch_result = page.execute_batch(actions, false).await.unwrap();
        assert_eq!(
            batch_result.succeeded, 2,
            "Both batch actions should succeed"
        );
        assert_eq!(batch_result.failed, 0);
        assert!(batch_result.all_succeeded());
        // The screenshot result should have data
        let screenshot_result = &batch_result.results[1];
        assert!(
            screenshot_result.data.is_some(),
            "Screenshot action should return data"
        );
        eprintln!(
            "  PASS: batch_mixed_actions (goto + screenshot, {}ms)",
            batch_result.total_duration_ms as u64
        );
    }

    // --- Test: profiler records spans ---
    let spans = browser.profiler().spans();
    assert!(
        spans.len() >= 3,
        "Expected >= 3 spans, got: {}",
        spans.len()
    );
    let summary = browser.profiler().export_ai_summary();
    assert!(
        summary.contains("RAYO PROFILE"),
        "Summary should contain RAYO PROFILE"
    );
    eprintln!("  PASS: profiler ({} spans recorded)", spans.len());

    eprintln!("ALL INTEGRATION TESTS PASSED");
}

/// Test that page_map truncation metadata is correct on a page with 100+ elements.
/// The EXTRACT_PAGE_MAP_JS caps at MAX_ELEMENTS=50, so total_interactive and truncated
/// should be set when the page has more than 50 interactive elements.
#[tokio::test]
async fn test_page_map_truncation() {
    if !chrome_available() {
        eprintln!("SKIP: Chrome not available");
        return;
    }

    let base_url = start_fixture_server().await;
    let browser = RayoBrowser::launch()
        .await
        .expect("Failed to launch browser");
    let page = browser.new_page().await.expect("Failed to create page");

    page.goto(&format!("{base_url}/many_elements.html"))
        .await
        .unwrap();

    let map = page.page_map(None).await.unwrap();

    assert_eq!(
        map.interactive.len(),
        50,
        "Should cap at 50 interactive elements, got: {}",
        map.interactive.len()
    );
    assert!(
        map.truncated == Some(true),
        "truncated should be true when page has >50 elements"
    );
    assert!(
        map.total_interactive.is_some(),
        "total_interactive should be set when truncated"
    );
    let total = map.total_interactive.unwrap();
    assert!(total > 50, "total_interactive should be >50, got: {total}");
    eprintln!(
        "  PASS: test_page_map_truncation (interactive={}, total={total}, truncated=true)",
        map.interactive.len()
    );
}

/// Test that disabled, readonly, and required element states are detected in page_map.
#[tokio::test]
async fn test_element_state_detection() {
    if !chrome_available() {
        eprintln!("SKIP: Chrome not available");
        return;
    }

    let base_url = start_fixture_server().await;
    let browser = RayoBrowser::launch()
        .await
        .expect("Failed to launch browser");
    let page = browser.new_page().await.expect("Failed to create page");

    page.goto(&format!("{base_url}/form.html")).await.unwrap();

    let map = page.page_map(None).await.unwrap();

    // Check readonly element
    let readonly_el = map
        .interactive
        .iter()
        .find(|e| e.selector.contains("readonly-input"));
    assert!(
        readonly_el.is_some(),
        "Should find readonly-input in page map"
    );
    assert!(
        readonly_el.unwrap().state.contains(&"readonly".to_string()),
        "readonly-input should have 'readonly' state, got: {:?}",
        readonly_el.unwrap().state
    );

    // Check disabled element
    let disabled_el = map
        .interactive
        .iter()
        .find(|e| e.selector.contains("disabled-input"));
    assert!(
        disabled_el.is_some(),
        "Should find disabled-input in page map"
    );
    assert!(
        disabled_el.unwrap().state.contains(&"disabled".to_string()),
        "disabled-input should have 'disabled' state, got: {:?}",
        disabled_el.unwrap().state
    );

    // Check required element
    let required_el = map
        .interactive
        .iter()
        .find(|e| e.selector.contains("required-input"));
    assert!(
        required_el.is_some(),
        "Should find required-input in page map"
    );
    assert!(
        required_el.unwrap().state.contains(&"required".to_string()),
        "required-input should have 'required' state, got: {:?}",
        required_el.unwrap().state
    );

    eprintln!("  PASS: test_element_state_detection");
}

/// Test that ARIA role equivalents populate item.text in both full and scoped page maps.
#[tokio::test]
async fn test_page_map_role_text_extraction() {
    if !chrome_available() {
        eprintln!("SKIP: Chrome not available");
        return;
    }

    let browser = RayoBrowser::launch()
        .await
        .expect("Failed to launch browser");
    let page = browser.new_page().await.expect("Failed to create page");

    page.goto("about:blank").await.unwrap();

    let html = r#"
        <main id="scope">
            <div role="button" id="role-button">Create project</div>
            <span role="link" id="role-link">Open docs</span>
            <div role="tab" id="role-tab">Settings</div>
        </main>
        <div role="button" id="outside-role-button">Outside action</div>
    "#;
    let script = format!(
        "document.title = 'Role Text Extraction'; document.body.innerHTML = {};",
        serde_json::to_string(html).unwrap()
    );
    page.evaluate(&script).await.unwrap();

    let map = page.page_map(None).await.unwrap();

    let role_button = map
        .interactive
        .iter()
        .find(|e| e.selector == "#role-button")
        .expect("full page map should include role button");
    assert_eq!(role_button.role.as_deref(), Some("button"));
    assert_eq!(role_button.text.as_deref(), Some("Create project"));

    let role_link = map
        .interactive
        .iter()
        .find(|e| e.selector == "#role-link")
        .expect("full page map should include role link");
    assert_eq!(role_link.role.as_deref(), Some("link"));
    assert_eq!(role_link.text.as_deref(), Some("Open docs"));

    let role_tab = map
        .interactive
        .iter()
        .find(|e| e.selector == "#role-tab")
        .expect("full page map should include role tab");
    assert_eq!(role_tab.role.as_deref(), Some("tab"));
    assert_eq!(role_tab.text.as_deref(), Some("Settings"));

    let scoped_map = page.page_map(Some("#scope")).await.unwrap();
    assert_eq!(
        scoped_map.interactive.len(),
        3,
        "scoped page map should only include in-scope role elements"
    );
    assert!(
        scoped_map
            .interactive
            .iter()
            .all(|e| e.selector != "#outside-role-button"),
        "scoped page map should exclude out-of-scope elements"
    );

    for (selector, expected_role, expected_text) in [
        ("#role-button", "button", "Create project"),
        ("#role-link", "link", "Open docs"),
        ("#role-tab", "tab", "Settings"),
    ] {
        let scoped = scoped_map
            .interactive
            .iter()
            .find(|e| e.selector == selector)
            .unwrap_or_else(|| panic!("scoped page map should include {selector}"));
        assert_eq!(scoped.role.as_deref(), Some(expected_role));
        assert_eq!(scoped.text.as_deref(), Some(expected_text));
    }

    eprintln!("  PASS: test_page_map_role_text_extraction");
}

/// Test that clicking a non-existent element returns RayoError::ElementNotFound.
#[tokio::test]
async fn test_click_nonexistent_element() {
    if !chrome_available() {
        eprintln!("SKIP: Chrome not available");
        return;
    }

    let base_url = start_fixture_server().await;
    let browser = RayoBrowser::launch()
        .await
        .expect("Failed to launch browser");
    let page = browser.new_page().await.expect("Failed to create page");

    page.goto(&format!("{base_url}/index.html")).await.unwrap();

    let result = page.click(Some("#does-not-exist-at-all-xyz"), None).await;
    assert!(result.is_err(), "Clicking non-existent element should fail");
    let err = result.unwrap_err();
    match &err {
        rayo_core::RayoError::ElementNotFound { selector } => {
            assert!(
                selector.contains("does-not-exist-at-all-xyz"),
                "Error should reference the selector, got: {selector}"
            );
        }
        other => {
            panic!("Expected RayoError::ElementNotFound, got: {other:?}");
        }
    }
    eprintln!("  PASS: test_click_nonexistent_element");
}

/// Test that a minimal page produces a page_map with an empty interactive array.
#[tokio::test]
async fn test_empty_page_map() {
    if !chrome_available() {
        eprintln!("SKIP: Chrome not available");
        return;
    }

    let browser = RayoBrowser::launch()
        .await
        .expect("Failed to launch browser");
    let page = browser.new_page().await.expect("Failed to create page");

    // Navigate to about:blank — no interactive elements
    page.goto("about:blank").await.unwrap();

    let map = page.page_map(None).await.unwrap();
    assert!(
        map.interactive.is_empty(),
        "about:blank should have no interactive elements, got: {}",
        map.interactive.len()
    );
    assert!(
        map.truncated.is_none() || map.truncated == Some(false),
        "about:blank should not be truncated"
    );
    eprintln!("  PASS: test_empty_page_map");
}

/// Test batch execution with a mix of valid and invalid actions.
/// With abort_on_failure=false, all actions should run and we should see
/// both succeeded > 0 and failed > 0.
#[tokio::test]
async fn test_batch_mixed_results() {
    if !chrome_available() {
        eprintln!("SKIP: Chrome not available");
        return;
    }

    let base_url = start_fixture_server().await;
    let browser = RayoBrowser::launch()
        .await
        .expect("Failed to launch browser");
    let page = browser.new_page().await.expect("Failed to create page");

    page.goto(&format!("{base_url}/index.html")).await.unwrap();

    let actions = vec![
        // Valid: take a screenshot (always works)
        BatchAction::Screenshot { full_page: false },
        // Invalid: click a non-existent element
        BatchAction::Click {
            target: rayo_core::batch::ActionTarget::Selector {
                selector: "#nonexistent-element-xyz".to_string(),
            },
        },
        // Valid: another screenshot
        BatchAction::Screenshot { full_page: false },
    ];

    let batch_result = page.execute_batch(actions, false).await.unwrap();
    assert!(
        batch_result.succeeded > 0,
        "Should have at least 1 succeeded action"
    );
    assert!(
        batch_result.failed > 0,
        "Should have at least 1 failed action"
    );
    assert_eq!(batch_result.succeeded, 2, "2 screenshots should succeed");
    assert_eq!(batch_result.failed, 1, "1 click on nonexistent should fail");
    assert!(!batch_result.all_succeeded());

    // Verify individual results
    assert!(
        batch_result.results[0].success,
        "First screenshot should succeed"
    );
    assert!(
        !batch_result.results[1].success,
        "Click nonexistent should fail"
    );
    assert!(
        batch_result.results[1].error.is_some(),
        "Failed action should have error message"
    );
    assert!(
        batch_result.results[2].success,
        "Third action (screenshot) should still run and succeed"
    );

    eprintln!(
        "  PASS: test_batch_mixed_results (succeeded={}, failed={})",
        batch_result.succeeded, batch_result.failed
    );
}

/// Test that wait_for_selector with visible=true times out on a hidden element.
#[tokio::test]
async fn test_wait_for_visibility() {
    if !chrome_available() {
        eprintln!("SKIP: Chrome not available");
        return;
    }

    let base_url = start_fixture_server().await;
    let browser = RayoBrowser::launch()
        .await
        .expect("Failed to launch browser");
    let page = browser.new_page().await.expect("Failed to create page");

    page.goto(&format!("{base_url}/index.html")).await.unwrap();

    // Inject a hidden element via JS
    page.evaluate(
        r#"(() => {
            const el = document.createElement('div');
            el.id = 'hidden-element';
            el.style.display = 'none';
            el.textContent = 'I am hidden';
            document.body.appendChild(el);
        })()"#,
    )
    .await
    .unwrap();

    // The element exists in DOM but is not visible.
    // wait_for_selector with visible=true and a short timeout should fail.
    let result = page.wait_for_selector("#hidden-element", 500, true).await;
    assert!(
        result.is_err(),
        "Should timeout waiting for a hidden element with visible=true"
    );

    // But with visible=false it should succeed (element exists in DOM).
    let result = page.wait_for_selector("#hidden-element", 500, false).await;
    assert!(
        result.is_ok(),
        "Should find hidden element with visible=false (DOM presence only)"
    );

    eprintln!("  PASS: test_wait_for_visibility");
}
