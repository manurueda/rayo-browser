//! Report generation — JSON and HTML output.

use crate::result::SuiteResult;
use std::path::Path;

/// Write a JSON report to disk.
pub fn write_json_report(result: &SuiteResult, path: &Path) -> Result<(), std::io::Error> {
    let json =
        serde_json::to_string_pretty(result).map_err(|e| std::io::Error::other(e.to_string()))?;
    std::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))?;
    std::fs::write(path, json)
}

/// Generate a self-contained HTML report.
pub fn generate_html_report(result: &SuiteResult) -> String {
    let pass_class = if result.pass { "pass" } else { "fail" };
    let status_text = if result.pass { "PASSED" } else { "FAILED" };

    let mut steps_html = String::new();
    for (i, step) in result.steps.iter().enumerate() {
        let step_class = if step.pass { "pass" } else { "fail" };
        let step_icon = if step.pass { "&#10003;" } else { "&#10007;" };

        steps_html.push_str(&format!(
            r#"<div class="step {step_class}">
  <div class="step-header">
    <span class="icon">{step_icon}</span>
    <span class="step-name">{name}</span>
    <span class="step-action">{action}</span>
    <span class="step-time">{duration}ms</span>
  </div>"#,
            name = html_escape(&step.name),
            action = step.action,
            duration = step.duration_ms,
        ));

        if let Some(ref error) = step.error {
            steps_html.push_str(&format!(
                r#"<div class="error">{}</div>"#,
                html_escape(error)
            ));
        }

        for assertion in &step.assertions {
            let a_class = if assertion.pass { "pass" } else { "fail" };
            let a_icon = if assertion.pass {
                "&#10003;"
            } else {
                "&#10007;"
            };
            steps_html.push_str(&format!(
                r#"<div class="assertion {a_class}">
    <span class="icon">{a_icon}</span>
    <span>{type_name}</span>
    {msg}
  </div>"#,
                type_name = assertion.assertion_type,
                msg = assertion
                    .message
                    .as_ref()
                    .map(|m| format!("<span class=\"msg\">{}</span>", html_escape(m)))
                    .unwrap_or_default(),
            ));
        }

        steps_html.push_str("</div>");

        if i < result.steps.len() - 1 {
            steps_html.push('\n');
        }
    }

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>rayo test: {name}</title>
<style>
body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; margin: 0; padding: 2rem; background: #0a0a0a; color: #e0e0e0; }}
h1 {{ margin: 0 0 1rem; font-size: 1.5rem; }}
.summary {{ padding: 1rem; border-radius: 8px; margin-bottom: 2rem; }}
.summary.pass {{ background: #0a2e1a; border: 1px solid #1a6b3a; }}
.summary.fail {{ background: #2e0a0a; border: 1px solid #6b1a1a; }}
.status {{ font-size: 1.2rem; font-weight: bold; }}
.status.pass {{ color: #4ade80; }}
.status.fail {{ color: #f87171; }}
.stats {{ color: #999; margin-top: 0.5rem; }}
.step {{ padding: 0.75rem 1rem; margin-bottom: 0.5rem; border-radius: 6px; background: #111; border: 1px solid #222; }}
.step.fail {{ border-color: #6b1a1a; }}
.step-header {{ display: flex; align-items: center; gap: 0.75rem; }}
.step-name {{ font-weight: 600; flex: 1; }}
.step-action {{ color: #888; font-size: 0.85rem; }}
.step-time {{ color: #666; font-size: 0.85rem; }}
.icon {{ font-size: 1.1rem; }}
.pass .icon {{ color: #4ade80; }}
.fail .icon {{ color: #f87171; }}
.error {{ color: #f87171; margin-top: 0.5rem; padding: 0.5rem; background: #1a0505; border-radius: 4px; font-size: 0.85rem; }}
.assertion {{ display: flex; align-items: center; gap: 0.5rem; margin-top: 0.4rem; padding-left: 1.5rem; font-size: 0.9rem; }}
.msg {{ color: #999; }}
</style>
</head>
<body>
<h1>rayo test: {name}</h1>
<div class="summary {pass_class}">
  <div class="status {pass_class}">{status_text}</div>
  <div class="stats">{passed}/{total} steps passed &middot; {duration}ms</div>
</div>
{steps_html}
</body>
</html>"#,
        name = html_escape(&result.name),
        passed = result.passed_steps,
        total = result.total_steps,
        duration = result.duration_ms,
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::result::{AssertionResult, StepResult, SuiteResult};

    #[test]
    fn html_report_contains_structure() {
        let result = SuiteResult {
            name: "Test Suite".into(),
            pass: true,
            total_steps: 2,
            passed_steps: 2,
            failed_steps: 0,
            duration_ms: 150,
            steps: vec![StepResult {
                name: "Navigate".into(),
                pass: true,
                duration_ms: 100,
                action: "navigate".into(),
                error: None,
                assertions: vec![AssertionResult {
                    assertion_type: "text_contains".into(),
                    pass: true,
                    message: None,
                    diff_report: None,
                    new_baseline: false,
                }],
                page_map: None,
            }],
            started_at: "2026-03-17T00:00:00Z".into(),
        };

        let html = generate_html_report(&result);
        assert!(html.contains("Test Suite"));
        assert!(html.contains("PASSED"));
        assert!(html.contains("Navigate"));
        assert!(html.contains("text_contains"));
    }
}
