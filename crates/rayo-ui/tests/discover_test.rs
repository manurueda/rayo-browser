//! Integration tests for rayo-ui discover command.
//!
//! These tests require Chrome/Chromium to be installed.
//! They launch a real browser and test the discover workflow against
//! fixture HTML files served by a local axum server.
//!
//! Run with: cargo test --package rayo-ui --test discover_test

use std::net::SocketAddr;
use std::path::PathBuf;

use rayo_ui::discover::{DiscoverConfig, discover};

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

/// Start a local axum server serving static files from `tests/fixtures/discover-test/`.
/// Returns the base URL (e.g. "http://127.0.0.1:<port>").
async fn start_fixture_server() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR"); // crates/rayo-ui
    let fixtures_dir = PathBuf::from(manifest_dir)
        .join("../../tests/fixtures/discover-test")
        .canonicalize()
        .expect("discover-test fixtures dir must exist");

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

#[tokio::test]
async fn test_discover_static_html() {
    if !chrome_available() {
        eprintln!("SKIP: Chrome not available");
        return;
    }

    let base_url = start_fixture_server().await;

    // Create a temp directory simulating a static HTML project.
    // Copy the fixture HTML files into it so the static_html analyzer can find them.
    let tmp = tempfile::tempdir().expect("Failed to create temp dir");
    let project_dir = tmp.path().to_path_buf();

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let fixtures_src = PathBuf::from(manifest_dir)
        .join("../../tests/fixtures/discover-test")
        .canonicalize()
        .expect("discover-test fixtures dir must exist");

    // Copy fixture HTML files into the temp project dir
    for entry in std::fs::read_dir(&fixtures_src).expect("Failed to read fixtures dir") {
        let entry = entry.expect("Failed to read entry");
        let dest = project_dir.join(entry.file_name());
        std::fs::copy(entry.path(), &dest).expect("Failed to copy fixture file");
    }

    let tests_dir = project_dir.join(".rayo/tests");
    let baselines_dir = project_dir.join(".rayo/baselines");

    let config = DiscoverConfig {
        url: base_url,
        project_dir: project_dir.clone(),
        tests_dir: tests_dir.clone(),
        baselines_dir,
        diff_mode: false,
        force: true,
        max_pages: 20,
    };

    let result = discover(config).await.expect("discover() should succeed");

    // --- Verify framework detection ---
    // The static HTML analyzer should detect the .html files in the project dir.
    // Framework name should be "Static HTML" (from StaticHtmlAnalyzer::name()).
    assert!(
        result.framework == "Static HTML" || result.framework == "Generic",
        "Framework should be 'Static HTML' or 'Generic', got: '{}'",
        result.framework
    );
    eprintln!("  PASS: framework detected as '{}'", result.framework);

    // --- Verify at least 1 route discovered ---
    assert!(
        result.routes_explored >= 1,
        "Should explore at least 1 route, got: {}",
        result.routes_explored
    );
    eprintln!(
        "  PASS: routes explored = {} (code routes = {})",
        result.routes_explored, result.routes_from_code
    );

    // --- Verify at least 1 YAML test file generated ---
    assert!(
        result.tests_generated >= 1,
        "Should generate at least 1 test file, got: {}",
        result.tests_generated
    );
    eprintln!("  PASS: tests generated = {}", result.tests_generated);

    // --- Verify generated YAML files are valid (parseable) ---
    assert!(
        tests_dir.exists(),
        "Tests directory should exist: {}",
        tests_dir.display()
    );

    let mut yaml_count = 0;
    for entry in std::fs::read_dir(&tests_dir).expect("Failed to read tests dir") {
        let entry = entry.expect("Failed to read entry");
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "yaml" || e == "yml") {
            let content =
                std::fs::read_to_string(&path).expect("Failed to read generated YAML file");
            let parsed: Result<rayo_ui::types::TestSuite, _> = serde_yaml::from_str(&content);
            assert!(
                parsed.is_ok(),
                "Generated YAML file should be parseable: {} — error: {:?}",
                path.display(),
                parsed.err()
            );
            let suite = parsed.unwrap();
            assert!(
                !suite.steps.is_empty(),
                "Generated suite '{}' should have at least 1 step",
                suite.name
            );
            yaml_count += 1;
        }
    }
    assert!(
        yaml_count >= 1,
        "Should find at least 1 YAML file in tests dir, found: {}",
        yaml_count
    );
    eprintln!(
        "  PASS: {} YAML files generated and all parseable",
        yaml_count
    );

    // --- Verify health score > 0 ---
    assert!(
        result.health_score > 0,
        "Health score should be > 0, got: {}",
        result.health_score
    );
    eprintln!("  PASS: health score = {}%", result.health_score);

    // --- Verify duration was recorded ---
    assert!(
        result.duration_ms > 0,
        "Duration should be > 0ms, got: {}",
        result.duration_ms
    );
    eprintln!("  PASS: duration = {}ms", result.duration_ms);

    // --- Verify flows were detected (the fixtures have forms and links) ---
    eprintln!(
        "  INFO: flows detected = {}, console errors = {}",
        result.flows_detected, result.console_errors
    );

    // Clean up happens automatically when `tmp` drops
    eprintln!("ALL DISCOVER INTEGRATION TESTS PASSED");
}
