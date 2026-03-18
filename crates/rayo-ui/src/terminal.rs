//! Rich terminal output for `rayo-ui run`.

use crate::result::SuiteResult;

const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
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
