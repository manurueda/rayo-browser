//! Tests for rayo-ui templates — StatsData, fmt_duration, template rendering.

use askama::Template;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use rayo_ui::result::{StepResult, SuiteResult};
use rayo_ui::templates::*;

// ---------------------------------------------------------------------------
// fmt_duration
// ---------------------------------------------------------------------------

#[test]
fn test_fmt_duration_milliseconds() {
    assert_eq!(fmt_duration(&0), "0ms");
    assert_eq!(fmt_duration(&1), "1ms");
    assert_eq!(fmt_duration(&150), "150ms");
    assert_eq!(fmt_duration(&999), "999ms");
}

#[test]
fn test_fmt_duration_seconds() {
    assert_eq!(fmt_duration(&1000), "1.0s");
    assert_eq!(fmt_duration(&1200), "1.2s");
    assert_eq!(fmt_duration(&2500), "2.5s");
    assert_eq!(fmt_duration(&10000), "10.0s");
}

#[test]
fn test_fmt_duration_boundary() {
    assert_eq!(fmt_duration(&1000), "1.0s");
    assert_eq!(fmt_duration(&999), "999ms");
}

#[test]
fn test_fmt_duration_large_values() {
    assert_eq!(fmt_duration(&60000), "60.0s");
    assert_eq!(fmt_duration(&123456), "123.5s");
}

// ---------------------------------------------------------------------------
// StatsData::default
// ---------------------------------------------------------------------------

#[test]
fn test_stats_data_default() {
    let stats = StatsData::default();
    assert_eq!(stats.pass_rate, 0.0);
    assert_eq!(stats.passed_suites, 0);
    assert_eq!(stats.total_suites, 0);
    assert_eq!(stats.total_steps, 0);
    assert_eq!(stats.total_duration_ms, 0);
}

// ---------------------------------------------------------------------------
// StatsData::from_results
// ---------------------------------------------------------------------------

#[test]
fn test_stats_data_from_empty_results() {
    let stats = StatsData::from_results(&[]);
    assert_eq!(stats.pass_rate, 0.0);
    assert_eq!(stats.total_suites, 0);
    assert_eq!(stats.passed_suites, 0);
    assert_eq!(stats.total_steps, 0);
    assert_eq!(stats.total_duration_ms, 0);
}

#[test]
fn test_stats_data_from_all_passing() {
    let results = vec![
        SuiteResult {
            name: "Suite A".into(),
            pass: true,
            total_steps: 5,
            passed_steps: 5,
            failed_steps: 0,
            duration_ms: 1000,
            steps: vec![],
            started_at: "2025-01-01T00:00:00Z".into(),
        },
        SuiteResult {
            name: "Suite B".into(),
            pass: true,
            total_steps: 3,
            passed_steps: 3,
            failed_steps: 0,
            duration_ms: 500,
            steps: vec![],
            started_at: "2025-01-01T00:00:00Z".into(),
        },
    ];

    let stats = StatsData::from_results(&results);
    assert_eq!(stats.total_suites, 2);
    assert_eq!(stats.passed_suites, 2);
    assert!((stats.pass_rate - 100.0).abs() < 0.01);
    assert_eq!(stats.total_steps, 8);
    assert_eq!(stats.total_duration_ms, 1500);
}

#[test]
fn test_stats_data_from_mixed_results() {
    let results = vec![
        SuiteResult {
            name: "Pass".into(),
            pass: true,
            total_steps: 4,
            passed_steps: 4,
            failed_steps: 0,
            duration_ms: 800,
            steps: vec![],
            started_at: "2025-01-01T00:00:00Z".into(),
        },
        SuiteResult {
            name: "Fail".into(),
            pass: false,
            total_steps: 6,
            passed_steps: 3,
            failed_steps: 3,
            duration_ms: 1200,
            steps: vec![],
            started_at: "2025-01-01T00:00:00Z".into(),
        },
    ];

    let stats = StatsData::from_results(&results);
    assert_eq!(stats.total_suites, 2);
    assert_eq!(stats.passed_suites, 1);
    assert!((stats.pass_rate - 50.0).abs() < 0.01);
    assert_eq!(stats.total_steps, 10);
    assert_eq!(stats.total_duration_ms, 2000);
}

#[test]
fn test_stats_data_from_all_failing() {
    let results = vec![SuiteResult {
        name: "Fail".into(),
        pass: false,
        total_steps: 2,
        passed_steps: 0,
        failed_steps: 2,
        duration_ms: 300,
        steps: vec![],
        started_at: "2025-01-01T00:00:00Z".into(),
    }];

    let stats = StatsData::from_results(&results);
    assert_eq!(stats.passed_suites, 0);
    assert_eq!(stats.pass_rate, 0.0);
}

// ---------------------------------------------------------------------------
// StatsData clone
// ---------------------------------------------------------------------------

#[test]
fn test_stats_data_clone() {
    let stats = StatsData {
        pass_rate: 75.0,
        passed_suites: 3,
        total_suites: 4,
        total_steps: 20,
        total_duration_ms: 5000,
    };
    let cloned = stats.clone();
    assert_eq!(cloned.total_suites, 4);
    assert!((cloned.pass_rate - 75.0).abs() < 0.01);
}

// ---------------------------------------------------------------------------
// SuiteSummary
// ---------------------------------------------------------------------------

#[test]
fn test_suite_summary_serialize() {
    let summary = SuiteSummary {
        name: "Login Flow".into(),
        path: ".rayo/tests/login.test.yaml".into(),
        steps: 5,
        has_setup: true,
        has_teardown: false,
    };

    let json = serde_json::to_string(&summary).unwrap();
    assert!(json.contains("\"name\":\"Login Flow\""));
    assert!(json.contains("\"steps\":5"));
    assert!(json.contains("\"has_setup\":true"));
    assert!(json.contains("\"has_teardown\":false"));
}

#[test]
fn test_suite_summary_clone() {
    let summary = SuiteSummary {
        name: "Test".into(),
        path: "/path".into(),
        steps: 3,
        has_setup: false,
        has_teardown: true,
    };
    let cloned = summary.clone();
    assert_eq!(cloned.name, "Test");
    assert_eq!(cloned.steps, 3);
    assert!(cloned.has_teardown);
}

// ---------------------------------------------------------------------------
// Template rendering
// ---------------------------------------------------------------------------

#[test]
fn test_live_template_renders() {
    let template = LiveTemplate;
    let rendered = template.render().unwrap();
    assert!(
        !rendered.is_empty(),
        "LiveTemplate should render non-empty HTML"
    );
}

#[test]
fn test_error_fragment_renders() {
    let template = ErrorFragment {
        message: "Something went wrong".into(),
    };
    let rendered = template.render().unwrap();
    assert!(rendered.contains("Something went wrong"));
}

#[test]
fn test_welcome_template_renders() {
    let template = WelcomeTemplate {
        chrome_available: true,
        discovering: false,
        tests_dir: ".rayo/tests".into(),
    };
    let rendered = template.render().unwrap();
    assert!(!rendered.is_empty());
}

#[test]
fn test_welcome_template_no_chrome() {
    let template = WelcomeTemplate {
        chrome_available: false,
        discovering: false,
        tests_dir: ".rayo/tests".into(),
    };
    let rendered = template.render().unwrap();
    assert!(!rendered.is_empty());
}

#[test]
fn test_live_progress_fragment_renders() {
    let template = LiveProgressFragment {
        suite_name: "Login Flow".into(),
        current: 3,
        total: 10,
        percent: 30.0,
    };
    let rendered = template.render().unwrap();
    assert!(rendered.contains("Login Flow"));
}

#[test]
fn test_stats_fragment_renders() {
    let template = StatsFragment {
        stats: StatsData {
            pass_rate: 85.0,
            passed_suites: 17,
            total_suites: 20,
            total_steps: 100,
            total_duration_ms: 5000,
        },
    };
    let rendered = template.render().unwrap();
    assert!(!rendered.is_empty());
}

#[test]
fn test_stats_fragment_with_default_stats() {
    let template = StatsFragment {
        stats: StatsData::default(),
    };
    let rendered = template.render().unwrap();
    assert!(!rendered.is_empty());
}

#[test]
fn test_available_suites_fragment_renders() {
    let template = AvailableSuitesFragment {
        suites: vec![
            SuiteSummary {
                name: "Suite A".into(),
                path: "a.yaml".into(),
                steps: 3,
                has_setup: false,
                has_teardown: false,
            },
            SuiteSummary {
                name: "Suite B".into(),
                path: "b.yaml".into(),
                steps: 5,
                has_setup: true,
                has_teardown: true,
            },
        ],
    };
    let rendered = template.render().unwrap();
    assert!(rendered.contains("Suite A"));
    assert!(rendered.contains("Suite B"));
}

#[test]
fn test_available_suites_fragment_empty() {
    let template = AvailableSuitesFragment { suites: vec![] };
    let rendered = template.render().unwrap();
    assert!(!rendered.is_empty());
}

#[test]
fn test_step_list_fragment_renders() {
    let template = StepListFragment {
        steps: vec![StepResult {
            name: "Click button".into(),
            pass: true,
            duration_ms: 150,
            action: "click".into(),
            error: None,
            assertions: vec![],
            page_map: None,
        }],
        suite_name: "Test Suite".into(),
    };
    let rendered = template.render().unwrap();
    assert!(rendered.contains("Click button"));
}

#[test]
fn test_step_list_fragment_empty_steps() {
    let template = StepListFragment {
        steps: vec![],
        suite_name: "Empty".into(),
    };
    let rendered = template.render().unwrap();
    assert!(!rendered.is_empty());
}

#[test]
fn test_suite_card_fragment_renders() {
    let template = SuiteCardFragment {
        result: SuiteResult {
            name: "My Suite".into(),
            pass: true,
            total_steps: 3,
            passed_steps: 3,
            failed_steps: 0,
            duration_ms: 800,
            steps: vec![],
            started_at: "2025-01-01T00:00:00Z".into(),
        },
    };
    let rendered = template.render().unwrap();
    assert!(rendered.contains("My Suite"));
}

#[test]
fn test_suite_list_fragment_renders() {
    let template = SuiteListFragment {
        results: vec![SuiteResult {
            name: "Suite X".into(),
            pass: false,
            total_steps: 5,
            passed_steps: 2,
            failed_steps: 3,
            duration_ms: 2000,
            steps: vec![],
            started_at: "2025-01-01T00:00:00Z".into(),
        }],
    };
    let rendered = template.render().unwrap();
    assert!(rendered.contains("Suite X"));
}

#[test]
fn test_live_event_fragment_renders() {
    let template = LiveEventFragment {
        step: StepResult {
            name: "Navigate".into(),
            pass: true,
            duration_ms: 200,
            action: "navigate".into(),
            error: None,
            assertions: vec![],
            page_map: None,
        },
        index: 0,
    };
    let rendered = template.render().unwrap();
    assert!(rendered.contains("Navigate"));
}

// ---------------------------------------------------------------------------
// HtmlTemplate into_response
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_html_template_success_response() {
    let template = HtmlTemplate(ErrorFragment {
        message: "Test error".into(),
    });
    let response = template.into_response();
    assert_eq!(response.status(), StatusCode::OK);
}
