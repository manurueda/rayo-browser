//! Integration tests for base_url feature.
//!
//! Tests that relative URLs in test YAML files resolve correctly against
//! a configured base_url. Uses a local axum server serving fixture HTML.
//!
//! Run with: cargo test --package rayo-ui --test base_url_test

use std::net::SocketAddr;
use std::path::PathBuf;

use rayo_ui::loader;
use rayo_ui::runner::{self, RunnerConfig};
use rayo_ui::types::{RayoConfig, TestSuite};

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
async fn start_fixture_server() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR"); // crates/rayo-ui
    let fixtures_dir = PathBuf::from(manifest_dir)
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

#[tokio::test]
async fn test_relative_url_with_base_url() {
    if !chrome_available() {
        eprintln!("SKIP: Chrome not available");
        return;
    }

    let base_url = start_fixture_server().await;

    // Test with relative path — should resolve to base_url + /index.html
    let yaml = r#"
name: Relative URL Test
steps:
  - name: Navigate to index via relative path
    navigate: /index.html
    assert:
      - text_contains: Test Page
"#;
    let suite: TestSuite = serde_yaml::from_str(yaml).unwrap();
    let config = RunnerConfig {
        baselines_dir: PathBuf::from("/tmp/rayo-test-baselines"),
        abort_on_failure: false,
        base_url: Some(base_url.clone()),
    };

    let result = runner::run_suite(&suite, &config, None)
        .await
        .expect("run_suite should succeed");
    assert!(
        result.pass,
        "Suite should pass with base_url resolving relative path: {:?}",
        result
            .steps
            .iter()
            .map(|s| (&s.name, &s.error))
            .collect::<Vec<_>>()
    );
    eprintln!("  PASS: relative URL /index.html resolved to {base_url}/index.html");
}

#[tokio::test]
async fn test_absolute_url_ignores_base_url() {
    if !chrome_available() {
        eprintln!("SKIP: Chrome not available");
        return;
    }

    let base_url = start_fixture_server().await;

    // Test with absolute URL — should NOT use base_url, should use the URL directly
    let yaml = format!(
        r#"
name: Absolute URL Test
steps:
  - name: Navigate with absolute URL
    navigate: {base_url}/index.html
    assert:
      - text_contains: Test Page
"#
    );
    let suite: TestSuite = serde_yaml::from_str(&yaml).unwrap();
    let config = RunnerConfig {
        baselines_dir: PathBuf::from("/tmp/rayo-test-baselines"),
        abort_on_failure: false,
        base_url: Some("http://localhost:9999".to_string()), // wrong port, should be ignored
    };

    let result = runner::run_suite(&suite, &config, None)
        .await
        .expect("run_suite should succeed");
    assert!(
        result.pass,
        "Suite should pass — absolute URL should ignore base_url: {:?}",
        result
            .steps
            .iter()
            .map(|s| (&s.name, &s.error))
            .collect::<Vec<_>>()
    );
    eprintln!("  PASS: absolute URL bypassed base_url");
}

#[tokio::test]
async fn test_relative_url_without_base_url_fails() {
    if !chrome_available() {
        eprintln!("SKIP: Chrome not available");
        return;
    }

    // Relative path with NO base_url — browser should fail (can't navigate to "/index.html")
    let yaml = r#"
name: No Base URL Test
steps:
  - name: Navigate with relative path and no base_url
    navigate: /index.html
"#;
    let suite: TestSuite = serde_yaml::from_str(yaml).unwrap();
    let config = RunnerConfig {
        baselines_dir: PathBuf::from("/tmp/rayo-test-baselines"),
        abort_on_failure: false,
        base_url: None,
    };

    let result = runner::run_suite(&suite, &config, None)
        .await
        .expect("run_suite should succeed (step fails, not the runner)");
    assert!(
        !result.pass,
        "Suite should fail when navigating to relative path without base_url"
    );
    eprintln!("  PASS: relative URL without base_url correctly fails");
}

#[tokio::test]
async fn test_root_path_with_base_url() {
    if !chrome_available() {
        eprintln!("SKIP: Chrome not available");
        return;
    }

    let base_url = start_fixture_server().await;

    // Navigate to "/" — should load the server root
    let yaml = r#"
name: Root Path Test
steps:
  - name: Navigate to root
    navigate: /
    assert:
      - text_contains: Test Page
"#;
    let suite: TestSuite = serde_yaml::from_str(yaml).unwrap();
    let config = RunnerConfig {
        baselines_dir: PathBuf::from("/tmp/rayo-test-baselines"),
        abort_on_failure: false,
        base_url: Some(base_url.clone()),
    };

    let result = runner::run_suite(&suite, &config, None)
        .await
        .expect("run_suite should succeed");
    assert!(
        result.pass,
        "Suite should pass navigating to / with base_url: {:?}",
        result
            .steps
            .iter()
            .map(|s| (&s.name, &s.error))
            .collect::<Vec<_>>()
    );
    eprintln!("  PASS: root path / resolved correctly");
}

#[tokio::test]
async fn test_multiple_relative_navigations() {
    if !chrome_available() {
        eprintln!("SKIP: Chrome not available");
        return;
    }

    let base_url = start_fixture_server().await;

    // Multiple steps with relative paths in one suite
    let yaml = r#"
name: Multi-Navigation Test
steps:
  - name: Navigate to index
    navigate: /index.html
    assert:
      - text_contains: Test Page
  - name: Navigate to form
    navigate: /form.html
    assert:
      - text_contains: Form
"#;
    let suite: TestSuite = serde_yaml::from_str(yaml).unwrap();
    let config = RunnerConfig {
        baselines_dir: PathBuf::from("/tmp/rayo-test-baselines"),
        abort_on_failure: false,
        base_url: Some(base_url.clone()),
    };

    let result = runner::run_suite(&suite, &config, None)
        .await
        .expect("run_suite should succeed");
    assert!(
        result.pass,
        "Suite should pass with multiple relative navigations: {:?}",
        result
            .steps
            .iter()
            .map(|s| (&s.name, &s.error))
            .collect::<Vec<_>>()
    );
    assert_eq!(result.passed_steps, 2);
    eprintln!("  PASS: multiple relative navigations worked");
}

#[tokio::test]
async fn test_batch_goto_with_base_url() {
    if !chrome_available() {
        eprintln!("SKIP: Chrome not available");
        return;
    }

    let base_url = start_fixture_server().await;

    // Batch goto with relative URL
    let yaml = r#"
name: Batch Goto Test
steps:
  - name: Batch navigate to form
    batch:
      - { action: goto, url: "/form.html" }
    assert:
      - text_contains: Form
"#;
    let suite: TestSuite = serde_yaml::from_str(yaml).unwrap();
    let config = RunnerConfig {
        baselines_dir: PathBuf::from("/tmp/rayo-test-baselines"),
        abort_on_failure: false,
        base_url: Some(base_url.clone()),
    };

    let result = runner::run_suite(&suite, &config, None)
        .await
        .expect("run_suite should succeed");
    assert!(
        result.pass,
        "Batch goto should resolve relative URL with base_url: {:?}",
        result
            .steps
            .iter()
            .map(|s| (&s.name, &s.error))
            .collect::<Vec<_>>()
    );
    eprintln!("  PASS: batch goto resolved relative URL");
}

#[test]
fn test_config_yaml_parsing() {
    let tmp = tempfile::tempdir().expect("Failed to create temp dir");
    let config_path = tmp.path().join("config.yaml");
    std::fs::write(&config_path, "base_url: http://localhost:3000\n").unwrap();

    let config = loader::load_config(tmp.path());
    assert_eq!(config.base_url.as_deref(), Some("http://localhost:3000"));
}

#[test]
fn test_config_yaml_missing() {
    let tmp = tempfile::tempdir().expect("Failed to create temp dir");
    let config = loader::load_config(tmp.path());
    assert!(config.base_url.is_none());
}

#[test]
fn test_config_yaml_empty() {
    let tmp = tempfile::tempdir().expect("Failed to create temp dir");
    let config_path = tmp.path().join("config.yaml");
    std::fs::write(&config_path, "{}\n").unwrap();

    let config = loader::load_config(tmp.path());
    assert!(config.base_url.is_none());
}

#[test]
fn test_config_yaml_extra_fields_ignored() {
    let tmp = tempfile::tempdir().expect("Failed to create temp dir");
    let config_path = tmp.path().join("config.yaml");
    std::fs::write(
        &config_path,
        "base_url: http://localhost:5000\nsome_future_field: true\n",
    )
    .unwrap();

    let config = loader::load_config(tmp.path());
    assert_eq!(config.base_url.as_deref(), Some("http://localhost:5000"));
}

#[test]
fn test_rayo_config_default() {
    let config = RayoConfig::default();
    assert!(config.base_url.is_none());
}
