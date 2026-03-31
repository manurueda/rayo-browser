//! Tests for rayo-ui discover module — resolve_url, DiscoverConfig, DiscoverResult.

use rayo_ui::discover::{DiscoverConfig, DiscoverResult};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// DiscoverConfig
// ---------------------------------------------------------------------------

#[test]
fn test_discover_config_defaults() {
    let config = DiscoverConfig {
        url: "http://localhost:3000".into(),
        project_dir: PathBuf::from("/tmp/project"),
        tests_dir: PathBuf::from("/tmp/project/.rayo/tests"),
        baselines_dir: PathBuf::from("/tmp/project/.rayo/baselines"),
        diff_mode: false,
        force: false,
        max_pages: 50,
    };

    assert_eq!(config.url, "http://localhost:3000");
    assert!(!config.diff_mode);
    assert!(!config.force);
    assert_eq!(config.max_pages, 50);
}

#[test]
fn test_discover_config_clone() {
    let config = DiscoverConfig {
        url: "http://localhost:3000".into(),
        project_dir: PathBuf::from("/tmp/project"),
        tests_dir: PathBuf::from("/tmp/project/.rayo/tests"),
        baselines_dir: PathBuf::from("/tmp/project/.rayo/baselines"),
        diff_mode: true,
        force: true,
        max_pages: 10,
    };

    let cloned = config.clone();
    assert_eq!(cloned.url, config.url);
    assert_eq!(cloned.project_dir, config.project_dir);
    assert_eq!(cloned.tests_dir, config.tests_dir);
    assert_eq!(cloned.baselines_dir, config.baselines_dir);
    assert_eq!(cloned.diff_mode, config.diff_mode);
    assert_eq!(cloned.force, config.force);
    assert_eq!(cloned.max_pages, config.max_pages);
}

// ---------------------------------------------------------------------------
// DiscoverResult
// ---------------------------------------------------------------------------

#[test]
fn test_discover_result_fields() {
    let result = DiscoverResult {
        framework: "Next.js".into(),
        routes_from_code: 10,
        routes_explored: 8,
        flows_detected: 5,
        tests_generated: 6,
        stories_discovered: 0,
        tests_passed: 4,
        tests_failed: 2,
        console_errors: 3,
        health_score: 75,
        duration_ms: 2500,
    };

    assert_eq!(result.framework, "Next.js");
    assert_eq!(result.routes_from_code, 10);
    assert_eq!(result.routes_explored, 8);
    assert_eq!(result.flows_detected, 5);
    assert_eq!(result.tests_generated, 6);
    assert_eq!(result.tests_passed, 4);
    assert_eq!(result.tests_failed, 2);
    assert_eq!(result.console_errors, 3);
    assert_eq!(result.health_score, 75);
    assert_eq!(result.duration_ms, 2500);
}

#[test]
fn test_discover_result_clone() {
    let result = DiscoverResult {
        framework: "Express".into(),
        routes_from_code: 3,
        routes_explored: 2,
        flows_detected: 1,
        tests_generated: 2,
        stories_discovered: 0,
        tests_passed: 1,
        tests_failed: 1,
        console_errors: 0,
        health_score: 90,
        duration_ms: 1000,
    };

    let cloned = result.clone();
    assert_eq!(cloned.framework, result.framework);
    assert_eq!(cloned.health_score, result.health_score);
    assert_eq!(cloned.duration_ms, result.duration_ms);
}

#[test]
fn test_discover_result_zero_values() {
    let result = DiscoverResult {
        framework: "Generic".into(),
        routes_from_code: 0,
        routes_explored: 0,
        flows_detected: 0,
        tests_generated: 0,
        stories_discovered: 0,
        tests_passed: 0,
        tests_failed: 0,
        console_errors: 0,
        health_score: 0,
        duration_ms: 0,
    };

    assert_eq!(result.routes_from_code, 0);
    assert_eq!(result.health_score, 0);
}

#[test]
fn test_discover_config_diff_mode() {
    let config = DiscoverConfig {
        url: "http://localhost:3000".into(),
        project_dir: PathBuf::from("."),
        tests_dir: PathBuf::from(".rayo/tests"),
        baselines_dir: PathBuf::from(".rayo/baselines"),
        diff_mode: true,
        force: false,
        max_pages: 20,
    };

    assert!(config.diff_mode);
    assert!(!config.force);
    assert_eq!(config.max_pages, 20);
}

#[test]
fn test_discover_config_force_mode() {
    let config = DiscoverConfig {
        url: "http://localhost:3000".into(),
        project_dir: PathBuf::from("."),
        tests_dir: PathBuf::from(".rayo/tests"),
        baselines_dir: PathBuf::from(".rayo/baselines"),
        diff_mode: false,
        force: true,
        max_pages: 100,
    };

    assert!(!config.diff_mode);
    assert!(config.force);
    assert_eq!(config.max_pages, 100);
}

#[test]
fn test_discover_result_debug_format() {
    let result = DiscoverResult {
        framework: "Rails".into(),
        routes_from_code: 5,
        routes_explored: 3,
        flows_detected: 2,
        tests_generated: 4,
        stories_discovered: 0,
        tests_passed: 3,
        tests_failed: 1,
        console_errors: 0,
        health_score: 80,
        duration_ms: 1500,
    };

    // Debug should be implemented and produce useful output
    let debug = format!("{:?}", result);
    assert!(debug.contains("Rails"));
    assert!(debug.contains("80"));
}
