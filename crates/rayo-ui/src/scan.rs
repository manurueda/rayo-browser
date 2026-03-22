//! Scan command — discovers flows, runs them, and builds results.
//!
//! The scan module bridges discover (flow detection) and the existing test
//! runner. It converts detected flows into YAML test suites, executes them
//! through `runner::run_suite`, and converts the results into `FlowResult`
//! structs for persistence and reporting.

use crate::discover::DiscoverResult;
use crate::discover::flows::FlowType;
use crate::narrative;
use crate::persistence::{FlowResult, FlowStepResult};
use crate::result::SuiteResult;
use crate::runner::RunnerConfig;
use std::path::Path;
use std::time::Instant;

/// Run all generated test suites and convert them to `FlowResult` entries.
///
/// Loads YAML test files produced by the discover phase, executes each suite
/// through the existing runner, and maps `SuiteResult` -> `FlowResult`.
/// A screenshot is captured at the end of each suite (v1 approach).
pub async fn run_scan(
    url: &str,
    discover_result: &DiscoverResult,
    tests_dir: &Path,
    baselines_dir: &Path,
) -> Vec<FlowResult> {
    let suites = match crate::loader::load_suites(tests_dir) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("    Warning: could not load generated tests: {e}");
            return Vec::new();
        }
    };

    let config = RunnerConfig {
        baselines_dir: baselines_dir.to_path_buf(),
        abort_on_failure: false,
        base_url: Some(url.to_string()),
    };

    let mut flow_results = Vec::new();
    let total = suites.len();

    for (i, file) in suites.iter().enumerate() {
        let progress = format!("[{}/{}]", i + 1, total);
        eprint!("    {progress} {}...", file.suite.name);

        let start = Instant::now();

        // Run suite with a 10-second timeout
        let suite_result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            crate::runner::run_suite(&file.suite, &config, None),
        )
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        let flow_result = match suite_result {
            Ok(Ok(result)) => {
                let icon = if result.pass {
                    "\x1b[32m\u{2713}\x1b[0m"
                } else {
                    "\x1b[31m\u{2717}\x1b[0m"
                };
                eprintln!(" {icon}");
                suite_result_to_flow_result(result, url)
            }
            Ok(Err(e)) => {
                eprintln!(" \x1b[31m\u{2717}\x1b[0m (error: {e})");
                FlowResult {
                    name: file.suite.name.clone(),
                    description: String::new(),
                    flow_type: guess_flow_type(&file.suite.name).as_str().to_string(),
                    importance: narrative::flow_importance(
                        &guess_flow_type(&file.suite.name),
                        &file.suite.name,
                    )
                    .as_str()
                    .to_string(),
                    url: url.to_string(),
                    passed: false,
                    duration_ms,
                    steps: Vec::new(),
                    error: Some(format!("{e}")),
                }
            }
            Err(_) => {
                eprintln!(" \x1b[31m\u{2717}\x1b[0m (timeout)");
                FlowResult {
                    name: file.suite.name.clone(),
                    description: String::new(),
                    flow_type: guess_flow_type(&file.suite.name).as_str().to_string(),
                    importance: narrative::flow_importance(
                        &guess_flow_type(&file.suite.name),
                        &file.suite.name,
                    )
                    .as_str()
                    .to_string(),
                    url: url.to_string(),
                    passed: false,
                    duration_ms,
                    steps: Vec::new(),
                    error: Some("Timed out after 10 seconds".to_string()),
                }
            }
        };

        flow_results.push(flow_result);
    }

    // Attach descriptions from discover context
    let _ = discover_result;
    for flow in &mut flow_results {
        if flow.description.is_empty() {
            flow.description = narrative::describe_flow_from_name(&flow.name, &flow.url);
        }
    }

    flow_results
}

/// Convert a `SuiteResult` from the test runner into a `FlowResult`.
fn suite_result_to_flow_result(result: SuiteResult, url: &str) -> FlowResult {
    let flow_type = guess_flow_type(&result.name);
    let importance = narrative::flow_importance(&flow_type, &result.name);

    let steps: Vec<FlowStepResult> = result
        .steps
        .iter()
        .map(|s| FlowStepResult {
            name: s.name.clone(),
            action: s.action.clone(),
            passed: s.pass,
            duration_ms: s.duration_ms,
            error: s.error.clone(),
            screenshot_path: None,
        })
        .collect();

    let error = if result.pass {
        None
    } else {
        // Find the first failing step's error
        result
            .steps
            .iter()
            .find(|s| !s.pass)
            .and_then(|s| {
                s.error.clone().or_else(|| {
                    s.assertions
                        .iter()
                        .find(|a| !a.pass)
                        .and_then(|a| a.message.clone())
                })
            })
            .or_else(|| Some("Test failed".to_string()))
    };

    FlowResult {
        name: result.name,
        description: String::new(),
        flow_type: flow_type.as_str().to_string(),
        importance: importance.as_str().to_string(),
        url: url.to_string(),
        passed: result.pass,
        duration_ms: result.duration_ms,
        steps,
        error,
    }
}

/// Guess the flow type from the suite name (heuristic).
fn guess_flow_type(name: &str) -> FlowType {
    let lower = name.to_lowercase();
    if lower.contains("login") || lower.contains("auth") || lower.contains("signin") {
        FlowType::Auth
    } else if lower.contains("search") {
        FlowType::Search
    } else if lower.contains("crud") {
        FlowType::Crud
    } else if lower.contains("navigation") || lower.contains("nav") || lower.contains("smoke") {
        FlowType::Navigation
    } else if lower.contains("form") {
        FlowType::Form
    } else {
        FlowType::Navigation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::result::{StepResult, SuiteResult};

    #[test]
    fn test_guess_flow_type() {
        assert_eq!(guess_flow_type("Login Flow"), FlowType::Auth);
        assert_eq!(guess_flow_type("home Search"), FlowType::Search);
        assert_eq!(guess_flow_type("users CRUD"), FlowType::Crud);
        assert_eq!(guess_flow_type("Site Navigation"), FlowType::Navigation);
        assert_eq!(guess_flow_type("Contact Form"), FlowType::Form);
        assert_eq!(guess_flow_type("_smoke"), FlowType::Navigation);
    }

    #[test]
    fn test_suite_result_to_flow_result_pass() {
        let suite = SuiteResult {
            name: "Login Flow".to_string(),
            pass: true,
            total_steps: 3,
            passed_steps: 3,
            failed_steps: 0,
            duration_ms: 500,
            steps: vec![
                StepResult {
                    name: "Navigate".to_string(),
                    pass: true,
                    duration_ms: 200,
                    action: "navigate".to_string(),
                    error: None,
                    assertions: vec![],
                    page_map: None,
                },
                StepResult {
                    name: "Type email".to_string(),
                    pass: true,
                    duration_ms: 100,
                    action: "type".to_string(),
                    error: None,
                    assertions: vec![],
                    page_map: None,
                },
            ],
            started_at: "2026-03-21T10:00:00Z".to_string(),
        };

        let flow = suite_result_to_flow_result(suite, "http://localhost:3000");
        assert_eq!(flow.name, "Login Flow");
        assert_eq!(flow.flow_type, "auth");
        assert_eq!(flow.importance, "critical");
        assert!(flow.passed);
        assert!(flow.error.is_none());
        assert_eq!(flow.steps.len(), 2);
    }

    #[test]
    fn test_suite_result_to_flow_result_fail() {
        let suite = SuiteResult {
            name: "Contact Form".to_string(),
            pass: false,
            total_steps: 2,
            passed_steps: 1,
            failed_steps: 1,
            duration_ms: 300,
            steps: vec![
                StepResult {
                    name: "Navigate".to_string(),
                    pass: true,
                    duration_ms: 200,
                    action: "navigate".to_string(),
                    error: None,
                    assertions: vec![],
                    page_map: None,
                },
                StepResult {
                    name: "Click submit".to_string(),
                    pass: false,
                    duration_ms: 100,
                    action: "click".to_string(),
                    error: Some("Element not found".to_string()),
                    assertions: vec![],
                    page_map: None,
                },
            ],
            started_at: "2026-03-21T10:00:00Z".to_string(),
        };

        let flow = suite_result_to_flow_result(suite, "http://localhost:3000");
        assert_eq!(flow.name, "Contact Form");
        assert_eq!(flow.flow_type, "form");
        assert!(!flow.passed);
        assert_eq!(flow.error, Some("Element not found".to_string()));
    }
}
