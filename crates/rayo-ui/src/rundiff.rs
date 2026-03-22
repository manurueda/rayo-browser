//! Run diff — compares two scan runs and produces a structured diff
//! showing regressions, improvements, and unchanged flows.

use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

use crate::persistence::{self, FlowResult, ScanResult};

/// Stable identity for a flow across runs: (flow_type, url).
type FlowKey = (String, String);

fn flow_key(flow: &FlowResult) -> FlowKey {
    (flow.flow_type.clone(), flow.url.clone())
}

/// Structured diff between two scan runs.
#[derive(Debug, Clone, Serialize)]
pub struct ScanDiff {
    pub previous_timestamp: String,
    pub current_timestamp: String,
    /// Positive = improved, negative = regressed (current - previous).
    pub health_score_change: i32,
    /// Flows that were passing but are now failing.
    pub flows_regressed: Vec<FlowDiff>,
    /// Flows that were failing but are now passing.
    pub flows_improved: Vec<FlowDiff>,
    /// Flows with the same pass/fail status.
    pub flows_unchanged: Vec<FlowDiff>,
    /// New flows not present in the previous run.
    pub flows_new: Vec<String>,
    /// Flows present in the previous run but not in the current one.
    pub flows_removed: Vec<String>,
}

/// Per-flow diff between two runs.
#[derive(Debug, Clone, Serialize)]
pub struct FlowDiff {
    pub name: String,
    pub flow_type: String,
    pub previous_passed: bool,
    pub current_passed: bool,
    /// Positive = slower, negative = faster.
    pub duration_change_ms: i64,
}

/// Compare two scan runs and produce a structured diff.
///
/// Flows are matched by their `(flow_type, url)` tuple, which serves as
/// a stable identity across runs.
pub fn compare_runs(previous: &ScanResult, current: &ScanResult) -> ScanDiff {
    let prev_map: HashMap<FlowKey, &FlowResult> =
        previous.flows.iter().map(|f| (flow_key(f), f)).collect();
    let curr_map: HashMap<FlowKey, &FlowResult> =
        current.flows.iter().map(|f| (flow_key(f), f)).collect();

    let mut flows_regressed = Vec::new();
    let mut flows_improved = Vec::new();
    let mut flows_unchanged = Vec::new();
    let mut flows_new = Vec::new();
    let mut flows_removed = Vec::new();

    // Walk current flows, matching against previous
    for curr_flow in &current.flows {
        let key = flow_key(curr_flow);
        match prev_map.get(&key) {
            Some(prev_flow) => {
                let diff = FlowDiff {
                    name: curr_flow.name.clone(),
                    flow_type: curr_flow.flow_type.clone(),
                    previous_passed: prev_flow.passed,
                    current_passed: curr_flow.passed,
                    duration_change_ms: curr_flow.duration_ms as i64 - prev_flow.duration_ms as i64,
                };

                if prev_flow.passed && !curr_flow.passed {
                    flows_regressed.push(diff);
                } else if !prev_flow.passed && curr_flow.passed {
                    flows_improved.push(diff);
                } else {
                    flows_unchanged.push(diff);
                }
            }
            None => {
                flows_new.push(curr_flow.name.clone());
            }
        }
    }

    // Find removed flows (in previous but not current)
    for prev_flow in &previous.flows {
        let key = flow_key(prev_flow);
        if !curr_map.contains_key(&key) {
            flows_removed.push(prev_flow.name.clone());
        }
    }

    let health_score_change = current.health_score as i32 - previous.health_score as i32;

    ScanDiff {
        previous_timestamp: previous.timestamp.clone(),
        current_timestamp: current.timestamp.clone(),
        health_score_change,
        flows_regressed,
        flows_improved,
        flows_unchanged,
        flows_new,
        flows_removed,
    }
}

/// Produce a human-readable summary of a scan diff.
///
/// Shows change direction and magnitude without absolute health scores.
/// Use [`diff_summary_with_scores`] when you have access to the original
/// health score values and want output like "health improved 75 -> 92".
pub fn diff_summary(diff: &ScanDiff) -> String {
    let has_changes = !diff.flows_regressed.is_empty()
        || !diff.flows_improved.is_empty()
        || !diff.flows_new.is_empty()
        || !diff.flows_removed.is_empty()
        || diff.health_score_change != 0;

    if !has_changes {
        return "No changes since last scan.".to_string();
    }

    let direction = if diff.health_score_change > 0 {
        format!("health improved by {} points", diff.health_score_change)
    } else if diff.health_score_change < 0 {
        format!(
            "health dropped by {} points",
            diff.health_score_change.abs()
        )
    } else {
        "health unchanged".to_string()
    };

    build_summary_parts(
        &direction,
        &diff.flows_regressed,
        &diff.flows_improved,
        &diff.flows_new,
        &diff.flows_removed,
    )
}

/// Produce a human-readable summary with absolute health scores.
///
/// This is the full-fidelity version that can show "health improved 75 -> 92".
pub fn diff_summary_with_scores(
    diff: &ScanDiff,
    previous_health: u32,
    current_health: u32,
) -> String {
    let has_changes = !diff.flows_regressed.is_empty()
        || !diff.flows_improved.is_empty()
        || !diff.flows_new.is_empty()
        || !diff.flows_removed.is_empty()
        || diff.health_score_change != 0;

    if !has_changes {
        return "No changes since last scan.".to_string();
    }

    let direction = if diff.health_score_change > 0 {
        format!("health improved {previous_health} -> {current_health}")
    } else if diff.health_score_change < 0 {
        format!("health dropped {previous_health} -> {current_health}")
    } else {
        format!("health unchanged at {current_health}")
    };

    build_summary_parts(
        &direction,
        &diff.flows_regressed,
        &diff.flows_improved,
        &diff.flows_new,
        &diff.flows_removed,
    )
}

/// Shared helper to build the narrative summary string.
fn build_summary_parts(
    direction: &str,
    regressed: &[FlowDiff],
    improved: &[FlowDiff],
    new: &[String],
    removed: &[String],
) -> String {
    let mut parts = vec![format!("Compared to last scan: {direction}.")];

    // Regressions
    let reg_count = regressed.len();
    if reg_count > 0 {
        let detail = regressed
            .iter()
            .map(|f| format!("{} (was passing, now failing)", f.name))
            .collect::<Vec<_>>()
            .join(", ");
        parts.push(format!(
            "{reg_count} regression{}: {detail}.",
            if reg_count == 1 { "" } else { "s" }
        ));
    } else {
        parts.push("0 regressions.".to_string());
    }

    // Improvements
    let imp_count = improved.len();
    if imp_count > 0 {
        parts.push(format!(
            "{imp_count} flow{} fixed.",
            if imp_count == 1 { "" } else { "s" }
        ));
    } else {
        parts.push("0 improvements.".to_string());
    }

    // New/removed
    if !new.is_empty() {
        parts.push(format!(
            "{} new flow{}.",
            new.len(),
            if new.len() == 1 { "" } else { "s" }
        ));
    }
    if !removed.is_empty() {
        parts.push(format!(
            "{} flow{} removed.",
            removed.len(),
            if removed.len() == 1 { "" } else { "s" }
        ));
    }

    parts.join(" ")
}

/// Compare the current scan against the most recent persisted run.
///
/// Returns `None` if no previous run exists or if loading fails.
pub fn compare_with_latest(current: &ScanResult, base_dir: &Path) -> Option<ScanDiff> {
    let previous = persistence::load_latest_run(base_dir).ok()??;
    Some(compare_runs(&previous, current))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::{FlowResult, ScanResult};

    fn make_flow(
        name: &str,
        flow_type: &str,
        url: &str,
        passed: bool,
        duration_ms: u64,
    ) -> FlowResult {
        FlowResult {
            name: name.to_string(),
            description: String::new(),
            flow_type: flow_type.to_string(),
            importance: "high".to_string(),
            url: url.to_string(),
            passed,
            duration_ms,
            steps: vec![],
            error: if passed {
                None
            } else {
                Some("failed".to_string())
            },
        }
    }

    fn make_scan(timestamp: &str, health_score: u32, flows: Vec<FlowResult>) -> ScanResult {
        let total = flows.len();
        let passed = flows.iter().filter(|f| f.passed).count();
        let failed = total - passed;
        let duration: u64 = flows.iter().map(|f| f.duration_ms).sum();
        ScanResult {
            url: "http://localhost:3000".to_string(),
            framework: "Next.js".to_string(),
            health_score,
            total_flows: total,
            passed_flows: passed,
            failed_flows: failed,
            total_duration_ms: duration,
            scan_duration_ms: duration,
            console_errors: 0,
            timestamp: timestamp.to_string(),
            flows,
        }
    }

    #[test]
    fn test_compare_runs_regressions_and_improvements() {
        let previous = make_scan(
            "2026-03-20T10:00:00Z",
            80,
            vec![
                make_flow("Login", "auth", "http://localhost:3000/login", true, 500),
                make_flow("Search", "form", "http://localhost:3000/search", false, 300),
                make_flow("Home", "navigation", "http://localhost:3000/", true, 200),
            ],
        );

        let current = make_scan(
            "2026-03-21T10:00:00Z",
            75,
            vec![
                // Login regressed: was passing, now failing
                make_flow("Login", "auth", "http://localhost:3000/login", false, 600),
                // Search improved: was failing, now passing
                make_flow("Search", "form", "http://localhost:3000/search", true, 250),
                // Home unchanged: still passing
                make_flow("Home", "navigation", "http://localhost:3000/", true, 210),
            ],
        );

        let diff = compare_runs(&previous, &current);

        assert_eq!(diff.previous_timestamp, "2026-03-20T10:00:00Z");
        assert_eq!(diff.current_timestamp, "2026-03-21T10:00:00Z");
        assert_eq!(diff.health_score_change, -5);

        // Regressions
        assert_eq!(diff.flows_regressed.len(), 1);
        assert_eq!(diff.flows_regressed[0].name, "Login");
        assert!(diff.flows_regressed[0].previous_passed);
        assert!(!diff.flows_regressed[0].current_passed);
        assert_eq!(diff.flows_regressed[0].duration_change_ms, 100); // 600 - 500

        // Improvements
        assert_eq!(diff.flows_improved.len(), 1);
        assert_eq!(diff.flows_improved[0].name, "Search");
        assert!(!diff.flows_improved[0].previous_passed);
        assert!(diff.flows_improved[0].current_passed);
        assert_eq!(diff.flows_improved[0].duration_change_ms, -50); // 250 - 300

        // Unchanged
        assert_eq!(diff.flows_unchanged.len(), 1);
        assert_eq!(diff.flows_unchanged[0].name, "Home");
        assert_eq!(diff.flows_unchanged[0].duration_change_ms, 10); // 210 - 200

        assert!(diff.flows_new.is_empty());
        assert!(diff.flows_removed.is_empty());
    }

    #[test]
    fn test_compare_runs_new_and_removed_flows() {
        let previous = make_scan(
            "2026-03-20T10:00:00Z",
            70,
            vec![
                make_flow("Login", "auth", "http://localhost:3000/login", true, 500),
                make_flow("Legacy", "form", "http://localhost:3000/legacy", true, 400),
            ],
        );

        let current = make_scan(
            "2026-03-21T10:00:00Z",
            80,
            vec![
                make_flow("Login", "auth", "http://localhost:3000/login", true, 480),
                make_flow(
                    "Dashboard",
                    "navigation",
                    "http://localhost:3000/dashboard",
                    true,
                    300,
                ),
            ],
        );

        let diff = compare_runs(&previous, &current);

        assert_eq!(diff.health_score_change, 10);

        // Login is unchanged (still passing)
        assert_eq!(diff.flows_unchanged.len(), 1);
        assert_eq!(diff.flows_unchanged[0].name, "Login");
        assert_eq!(diff.flows_unchanged[0].duration_change_ms, -20); // 480 - 500

        // Dashboard is new
        assert_eq!(diff.flows_new, vec!["Dashboard"]);

        // Legacy was removed
        assert_eq!(diff.flows_removed, vec!["Legacy"]);

        assert!(diff.flows_regressed.is_empty());
        assert!(diff.flows_improved.is_empty());
    }

    #[test]
    fn test_compare_runs_empty_runs() {
        let previous = make_scan("2026-03-20T10:00:00Z", 0, vec![]);
        let current = make_scan("2026-03-21T10:00:00Z", 0, vec![]);

        let diff = compare_runs(&previous, &current);

        assert_eq!(diff.health_score_change, 0);
        assert!(diff.flows_regressed.is_empty());
        assert!(diff.flows_improved.is_empty());
        assert!(diff.flows_unchanged.is_empty());
        assert!(diff.flows_new.is_empty());
        assert!(diff.flows_removed.is_empty());
    }

    #[test]
    fn test_compare_runs_all_new_flows() {
        let previous = make_scan("2026-03-20T10:00:00Z", 0, vec![]);
        let current = make_scan(
            "2026-03-21T10:00:00Z",
            90,
            vec![
                make_flow("Login", "auth", "http://localhost:3000/login", true, 500),
                make_flow("Search", "form", "http://localhost:3000/search", true, 300),
            ],
        );

        let diff = compare_runs(&previous, &current);

        assert_eq!(diff.flows_new.len(), 2);
        assert!(diff.flows_regressed.is_empty());
        assert!(diff.flows_improved.is_empty());
        assert!(diff.flows_unchanged.is_empty());
        assert!(diff.flows_removed.is_empty());
    }

    #[test]
    fn test_compare_runs_all_removed_flows() {
        let previous = make_scan(
            "2026-03-20T10:00:00Z",
            90,
            vec![
                make_flow("Login", "auth", "http://localhost:3000/login", true, 500),
                make_flow("Search", "form", "http://localhost:3000/search", true, 300),
            ],
        );
        let current = make_scan("2026-03-21T10:00:00Z", 0, vec![]);

        let diff = compare_runs(&previous, &current);

        assert_eq!(diff.flows_removed.len(), 2);
        assert!(diff.flows_regressed.is_empty());
        assert!(diff.flows_improved.is_empty());
        assert!(diff.flows_unchanged.is_empty());
        assert!(diff.flows_new.is_empty());
    }

    #[test]
    fn test_compare_runs_flow_identity_by_type_and_url() {
        // Same name but different (flow_type, url) should be treated as different flows
        let previous = make_scan(
            "2026-03-20T10:00:00Z",
            80,
            vec![make_flow(
                "Login",
                "auth",
                "http://localhost:3000/login",
                true,
                500,
            )],
        );

        let current = make_scan(
            "2026-03-21T10:00:00Z",
            80,
            vec![make_flow(
                "Login",
                "form",
                "http://localhost:3000/login-v2",
                true,
                500,
            )],
        );

        let diff = compare_runs(&previous, &current);

        // The old "Login" is removed and the new "Login" is new — different identity
        assert_eq!(diff.flows_new, vec!["Login"]);
        assert_eq!(diff.flows_removed, vec!["Login"]);
        assert!(diff.flows_unchanged.is_empty());
    }

    #[test]
    fn test_diff_summary_no_changes() {
        let diff = ScanDiff {
            previous_timestamp: "2026-03-20T10:00:00Z".to_string(),
            current_timestamp: "2026-03-21T10:00:00Z".to_string(),
            health_score_change: 0,
            flows_regressed: vec![],
            flows_improved: vec![],
            flows_unchanged: vec![FlowDiff {
                name: "Login".to_string(),
                flow_type: "auth".to_string(),
                previous_passed: true,
                current_passed: true,
                duration_change_ms: 0,
            }],
            flows_new: vec![],
            flows_removed: vec![],
        };

        assert_eq!(diff_summary(&diff), "No changes since last scan.");
    }

    #[test]
    fn test_diff_summary_with_scores_improvement() {
        let diff = ScanDiff {
            previous_timestamp: "2026-03-20T10:00:00Z".to_string(),
            current_timestamp: "2026-03-21T10:00:00Z".to_string(),
            health_score_change: 17,
            flows_regressed: vec![],
            flows_improved: vec![
                FlowDiff {
                    name: "Login".to_string(),
                    flow_type: "auth".to_string(),
                    previous_passed: false,
                    current_passed: true,
                    duration_change_ms: -100,
                },
                FlowDiff {
                    name: "Search".to_string(),
                    flow_type: "form".to_string(),
                    previous_passed: false,
                    current_passed: true,
                    duration_change_ms: -50,
                },
            ],
            flows_unchanged: vec![],
            flows_new: vec![],
            flows_removed: vec![],
        };

        let summary = diff_summary_with_scores(&diff, 75, 92);
        assert!(summary.contains("health improved 75 -> 92"));
        assert!(summary.contains("2 flows fixed"));
        assert!(summary.contains("0 regressions"));
    }

    #[test]
    fn test_diff_summary_with_scores_regression() {
        let diff = ScanDiff {
            previous_timestamp: "2026-03-20T10:00:00Z".to_string(),
            current_timestamp: "2026-03-21T10:00:00Z".to_string(),
            health_score_change: -17,
            flows_regressed: vec![FlowDiff {
                name: "Login Flow".to_string(),
                flow_type: "auth".to_string(),
                previous_passed: true,
                current_passed: false,
                duration_change_ms: 200,
            }],
            flows_improved: vec![],
            flows_unchanged: vec![],
            flows_new: vec![],
            flows_removed: vec![],
        };

        let summary = diff_summary_with_scores(&diff, 92, 75);
        assert!(summary.contains("health dropped 92 -> 75"));
        assert!(summary.contains("1 regression:"));
        assert!(summary.contains("Login Flow (was passing, now failing)"));
        assert!(summary.contains("0 improvements"));
    }

    #[test]
    fn test_diff_summary_with_new_and_removed() {
        let diff = ScanDiff {
            previous_timestamp: "2026-03-20T10:00:00Z".to_string(),
            current_timestamp: "2026-03-21T10:00:00Z".to_string(),
            health_score_change: 5,
            flows_regressed: vec![],
            flows_improved: vec![],
            flows_unchanged: vec![],
            flows_new: vec!["Dashboard".to_string(), "Settings".to_string()],
            flows_removed: vec!["Legacy".to_string()],
        };

        let summary = diff_summary_with_scores(&diff, 80, 85);
        assert!(summary.contains("2 new flows"));
        assert!(summary.contains("1 flow removed"));
    }

    #[test]
    fn test_compare_with_latest_no_previous() {
        let tmp = tempfile::tempdir().unwrap();
        let current = make_scan("2026-03-21T10:00:00Z", 80, vec![]);

        let result = compare_with_latest(&current, tmp.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_compare_with_latest_has_previous() {
        let tmp = tempfile::tempdir().unwrap();

        // Save a previous run
        let previous = make_scan(
            "2026-03-20T10:00:00Z",
            70,
            vec![make_flow(
                "Login",
                "auth",
                "http://localhost:3000/login",
                false,
                500,
            )],
        );
        persistence::save_run(&previous, tmp.path()).unwrap();

        // Current run with the flow now passing
        let current = make_scan(
            "2026-03-21T10:00:00Z",
            90,
            vec![make_flow(
                "Login",
                "auth",
                "http://localhost:3000/login",
                true,
                400,
            )],
        );

        let diff = compare_with_latest(&current, tmp.path()).unwrap();
        assert_eq!(diff.health_score_change, 20);
        assert_eq!(diff.flows_improved.len(), 1);
        assert_eq!(diff.flows_improved[0].name, "Login");
        assert_eq!(diff.flows_improved[0].duration_change_ms, -100);
        assert!(diff.flows_regressed.is_empty());
    }

    #[test]
    fn test_diff_summary_unchanged_health_with_new_flows() {
        let diff = ScanDiff {
            previous_timestamp: "2026-03-20T10:00:00Z".to_string(),
            current_timestamp: "2026-03-21T10:00:00Z".to_string(),
            health_score_change: 0,
            flows_regressed: vec![],
            flows_improved: vec![],
            flows_unchanged: vec![],
            flows_new: vec!["NewFlow".to_string()],
            flows_removed: vec![],
        };

        let summary = diff_summary_with_scores(&diff, 80, 80);
        assert!(summary.contains("health unchanged at 80"));
        assert!(summary.contains("1 new flow."));
    }

    #[test]
    fn test_compare_runs_unchanged_both_failing() {
        let previous = make_scan(
            "2026-03-20T10:00:00Z",
            50,
            vec![make_flow(
                "Broken",
                "form",
                "http://localhost:3000/broken",
                false,
                1000,
            )],
        );
        let current = make_scan(
            "2026-03-21T10:00:00Z",
            50,
            vec![make_flow(
                "Broken",
                "form",
                "http://localhost:3000/broken",
                false,
                1100,
            )],
        );

        let diff = compare_runs(&previous, &current);

        assert!(diff.flows_regressed.is_empty());
        assert!(diff.flows_improved.is_empty());
        assert_eq!(diff.flows_unchanged.len(), 1);
        assert!(!diff.flows_unchanged[0].previous_passed);
        assert!(!diff.flows_unchanged[0].current_passed);
        assert_eq!(diff.flows_unchanged[0].duration_change_ms, 100);
    }
}
