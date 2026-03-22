//! Rich terminal output for `rayo-ui run` and `rayo-ui scan`.

use crate::persistence::ScanResult;
use crate::result::SuiteResult;
use crate::rundiff::ScanDiff;

const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

/// Print a formatted summary of test results.
pub fn print_run_summary(results: &[SuiteResult], verbose: bool) {
    let total_suites = results.len();
    let total_steps: usize = results.iter().map(|r| r.total_steps).sum();

    eprintln!();
    eprintln!(
        "  {BOLD}\u{26a1} rayo-ui{RESET} \u{2014} {total_suites} suite{}, {total_steps} step{}",
        if total_suites != 1 { "s" } else { "" },
        if total_steps != 1 { "s" } else { "" },
    );
    eprintln!(
        "  {DIM}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}{RESET}"
    );

    for result in results {
        let icon = if result.pass {
            format!("{GREEN}\u{2713}{RESET}")
        } else {
            format!("{RED}\u{2717}{RESET}")
        };
        let name_color = if result.pass { GREEN } else { RED };
        let dur = fmt_dur(result.duration_ms);

        eprintln!(
            "  {icon}  {name_color}{name}{RESET}  {DIM}{passed}/{total}{RESET}  {DIM}{dur}{RESET}",
            name = result.name,
            passed = result.passed_steps,
            total = result.total_steps,
        );

        // Failed suites: show failure details
        if !result.pass {
            for step in &result.steps {
                if !step.pass {
                    eprintln!("     {RED}\u{2514}\u{2500} {}{RESET}", step.name);
                    if let Some(ref err) = step.error {
                        eprintln!("        {DIM}{err}{RESET}");
                    }
                    for a in &step.assertions {
                        if !a.pass
                            && let Some(ref msg) = a.message
                        {
                            eprintln!("        {DIM}{}: {msg}{RESET}", a.assertion_type);
                        }
                    }
                }
            }
        }

        // Verbose: show all steps
        if verbose {
            for step in &result.steps {
                let s_icon = if step.pass { "\u{2713}" } else { "\u{2717}" };
                let s_color = if step.pass { GREEN } else { RED };
                eprintln!(
                    "     {s_color}{s_icon}{RESET}  {DIM}{name}  {dur}{RESET}",
                    name = step.name,
                    dur = fmt_dur(step.duration_ms),
                );
                for a in &step.assertions {
                    let a_icon = if a.pass { "\u{2713}" } else { "\u{2717}" };
                    let a_color = if a.pass { GREEN } else { RED };
                    eprint!("       {a_color}{a_icon}{RESET}  {DIM}{}", a.assertion_type);
                    if let Some(ref msg) = a.message {
                        eprint!(" \u{2014} {msg}");
                    }
                    if a.new_baseline {
                        eprint!(" (new baseline)");
                    }
                    eprintln!("{RESET}");
                }
            }
        }
    }

    // Footer
    let total_passed: usize = results.iter().map(|r| r.passed_steps).sum();
    let total_failed: usize = results.iter().map(|r| r.failed_steps).sum();
    let total_duration_ms: u64 = results.iter().map(|r| r.duration_ms).sum();

    eprintln!(
        "  {DIM}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}{RESET}"
    );

    let suites_passed = results.iter().filter(|r| r.pass).count();
    let suites_failed = total_suites - suites_passed;

    if suites_failed == 0 {
        eprintln!(
            "  {GREEN}{BOLD}ALL PASSED{RESET}  {DIM}{total_passed} passed \u{00b7} {total_suites} suites \u{00b7} {}{RESET}",
            fmt_dur(total_duration_ms),
        );
    } else {
        eprintln!(
            "  {RED}{BOLD}{suites_failed} FAILED{RESET}  {DIM}{total_passed} passed \u{00b7} {total_failed} failed \u{00b7} {total_suites} suites \u{00b7} {}{RESET}",
            fmt_dur(total_duration_ms),
        );
    }
    eprintln!();
}

fn fmt_dur(ms: u64) -> String {
    if ms >= 1000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        format!("{ms}ms")
    }
}

/// Print a formatted summary of a scan result (consumer QA dashboard).
///
/// Shows health score with progress bar, flow list sorted by importance,
/// optional diff comparison, and speed callout.
pub fn print_scan_summary(scan: &ScanResult, diff: Option<&ScanDiff>, url: &str) {
    eprintln!();
    eprintln!("  {BOLD}\u{26a1} rayo scan complete{RESET} \u{2014} {url}");
    eprintln!();

    // Health score with progress bar
    let health = scan.health_score;
    let health_color = if health >= 80 {
        GREEN
    } else if health >= 50 {
        YELLOW
    } else {
        RED
    };
    let filled = (health / 10) as usize;
    let empty = 10 - filled;
    let bar: String = "\u{2588}".repeat(filled) + &"\u{2591}".repeat(empty);
    let health_label = if health >= 80 {
        "Healthy"
    } else if health >= 50 {
        "Needs Work"
    } else {
        "Critical"
    };
    eprintln!("  Health: {health_color}{health}/100 {bar} {health_label}{RESET}");

    // Diff line
    if let Some(d) = diff {
        let prev_health = (health as i32 - d.health_score_change) as u32;
        let arrow = if d.health_score_change > 0 {
            format!("{GREEN}\u{25b2}{RESET}")
        } else if d.health_score_change < 0 {
            format!("{RED}\u{25bc}{RESET}")
        } else {
            format!("{DIM}\u{25cf}{RESET}")
        };
        let change_sign = if d.health_score_change > 0 {
            format!("+{}", d.health_score_change)
        } else {
            format!("{}", d.health_score_change)
        };
        eprintln!("  vs last scan: {arrow} {prev_health} \u{2192} {health} ({change_sign})");
    }
    eprintln!();

    // Sort flows: by importance (Critical first), then failures before passes
    let mut sorted_flows: Vec<&crate::persistence::FlowResult> = scan.flows.iter().collect();
    sorted_flows.sort_by(|a, b| {
        let imp_a = importance_rank(&a.importance);
        let imp_b = importance_rank(&b.importance);
        // Higher importance first
        imp_b.cmp(&imp_a).then_with(|| {
            // Failures before passes within same importance
            a.passed.cmp(&b.passed)
        })
    });

    for flow in &sorted_flows {
        let icon = if flow.passed {
            format!("{GREEN}\u{2713}{RESET}")
        } else {
            format!("{RED}\u{2717}{RESET}")
        };

        let imp_color = match flow.importance.as_str() {
            "critical" => RED,
            "high" => YELLOW,
            "medium" => DIM,
            _ => DIM,
        };
        let imp_label = capitalize(&flow.importance);
        let dur = fmt_dur(flow.duration_ms);
        let status = if flow.passed {
            format!("{GREEN}passed{RESET}")
        } else {
            format!("{RED}{BOLD}FAILED{RESET}")
        };

        eprintln!(
            "  {icon} {name:<20} {imp_color}[{imp_label}]{RESET}  {DIM}{dur}{RESET}   {status}",
            name = flow.name,
        );

        // Show error detail for failed flows
        if !flow.passed
            && let Some(ref err) = flow.error
        {
            eprintln!("    {RED}\u{2514} {err}{RESET}");
        }
    }

    // Console errors warning
    if scan.console_errors > 0 {
        eprintln!();
        eprintln!(
            "  {YELLOW}\u{26a0} {} console error{} detected{RESET}",
            scan.console_errors,
            if scan.console_errors == 1 { "" } else { "s" },
        );
    }

    eprintln!();
    eprintln!(
        "  {DIM}{}{RESET}",
        crate::narrative::speed_callout(scan.scan_duration_ms),
    );
    eprintln!();
}

/// Map importance string to a sort rank (higher = more important).
fn importance_rank(importance: &str) -> u8 {
    match importance {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

/// Capitalize the first letter of a string.
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => {
            let upper: String = first.to_uppercase().collect();
            format!("{upper}{}", chars.as_str())
        }
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::{FlowResult, ScanResult};
    use crate::rundiff::{FlowDiff, ScanDiff};

    fn sample_scan() -> ScanResult {
        ScanResult {
            url: "http://localhost:3000".to_string(),
            framework: "Next.js".to_string(),
            health_score: 85,
            total_flows: 3,
            passed_flows: 2,
            failed_flows: 1,
            total_duration_ms: 3200,
            scan_duration_ms: 3200,
            console_errors: 2,
            timestamp: "2026-03-21T10:00:00Z".to_string(),
            flows: vec![
                FlowResult {
                    name: "Login Flow".to_string(),
                    description: String::new(),
                    flow_type: "auth".to_string(),
                    importance: "critical".to_string(),
                    url: "http://localhost:3000/login".to_string(),
                    passed: true,
                    duration_ms: 1200,
                    steps: vec![],
                    error: None,
                },
                FlowResult {
                    name: "Checkout Flow".to_string(),
                    description: String::new(),
                    flow_type: "form".to_string(),
                    importance: "high".to_string(),
                    url: "http://localhost:3000/checkout".to_string(),
                    passed: false,
                    duration_ms: 2100,
                    steps: vec![],
                    error: Some("Submit button unresponsive".to_string()),
                },
                FlowResult {
                    name: "Site Navigation".to_string(),
                    description: String::new(),
                    flow_type: "navigation".to_string(),
                    importance: "low".to_string(),
                    url: "http://localhost:3000/".to_string(),
                    passed: true,
                    duration_ms: 890,
                    steps: vec![],
                    error: None,
                },
            ],
        }
    }

    #[test]
    fn test_importance_rank() {
        assert!(importance_rank("critical") > importance_rank("high"));
        assert!(importance_rank("high") > importance_rank("medium"));
        assert!(importance_rank("medium") > importance_rank("low"));
        assert!(importance_rank("low") > importance_rank("unknown"));
    }

    #[test]
    fn test_capitalize() {
        assert_eq!(capitalize("critical"), "Critical");
        assert_eq!(capitalize("high"), "High");
        assert_eq!(capitalize(""), "");
    }

    #[test]
    fn test_print_scan_summary_no_panic() {
        let scan = sample_scan();
        // Should not panic with or without diff
        print_scan_summary(&scan, None, "http://localhost:3000");

        let diff = ScanDiff {
            previous_timestamp: "2026-03-20T10:00:00Z".to_string(),
            current_timestamp: "2026-03-21T10:00:00Z".to_string(),
            health_score_change: 10,
            flows_regressed: vec![],
            flows_improved: vec![FlowDiff {
                name: "Login".to_string(),
                flow_type: "auth".to_string(),
                previous_passed: false,
                current_passed: true,
                duration_change_ms: -100,
            }],
            flows_unchanged: vec![],
            flows_new: vec![],
            flows_removed: vec![],
        };
        print_scan_summary(&scan, Some(&diff), "http://localhost:3000");
    }

    #[test]
    fn test_print_scan_summary_zero_health() {
        let scan = ScanResult {
            url: "http://localhost:3000".to_string(),
            framework: "Static".to_string(),
            health_score: 0,
            total_flows: 0,
            passed_flows: 0,
            failed_flows: 0,
            total_duration_ms: 100,
            scan_duration_ms: 100,
            console_errors: 0,
            timestamp: "2026-03-21T10:00:00Z".to_string(),
            flows: vec![],
        };
        print_scan_summary(&scan, None, "http://localhost:3000");
    }

    #[test]
    fn test_print_scan_summary_perfect_health() {
        let scan = ScanResult {
            url: "http://localhost:3000".to_string(),
            framework: "Next.js".to_string(),
            health_score: 100,
            total_flows: 1,
            passed_flows: 1,
            failed_flows: 0,
            total_duration_ms: 500,
            scan_duration_ms: 500,
            console_errors: 0,
            timestamp: "2026-03-21T10:00:00Z".to_string(),
            flows: vec![FlowResult {
                name: "Login".to_string(),
                description: String::new(),
                flow_type: "auth".to_string(),
                importance: "critical".to_string(),
                url: "http://localhost:3000/login".to_string(),
                passed: true,
                duration_ms: 500,
                steps: vec![],
                error: None,
            }],
        };
        print_scan_summary(&scan, None, "http://localhost:3000");
    }
}
