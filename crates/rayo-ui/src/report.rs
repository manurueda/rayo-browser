//! Report generation — JSON and HTML output.

use crate::persistence::{FlowResult, ScanResult};
use crate::result::SuiteResult;
use crate::rundiff::ScanDiff;
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
            action = html_escape(&step.action),
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
                type_name = html_escape(&assertion.assertion_type),
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
        .replace('\'', "&#x27;")
}

// ---------------------------------------------------------------------------
// Scan report — consumer-grade QA dashboard
// ---------------------------------------------------------------------------

/// HTML-escape a string for safe embedding in the scan report.
///
/// Uses the same escape logic as [`crate::narrative::html_escape`] (ampersand,
/// angle brackets, double-quotes, single-quotes).
fn scan_html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Inline screenshot files as base64 data URIs in a [`ScanResult`].
///
/// For every [`FlowStepResult`] that has a `screenshot_path`, the file is read
/// from `run_dir / screenshot_path`, base64-encoded, and the path is replaced
/// with a `data:image/jpeg;base64,...` data URI.  Steps whose file cannot be
/// read are silently skipped (the path is left unchanged).
pub fn inline_screenshots(scan: &mut ScanResult, run_dir: &Path) {
    use base64::Engine;
    let engine = base64::engine::general_purpose::STANDARD;

    for flow in &mut scan.flows {
        for step in &mut flow.steps {
            if let Some(ref path) = step.screenshot_path {
                // Skip paths that are already data URIs
                if path.starts_with("data:") {
                    continue;
                }
                let full_path = run_dir.join(path);
                if let Ok(bytes) = std::fs::read(&full_path) {
                    let b64 = engine.encode(&bytes);
                    step.screenshot_path = Some(format!("data:image/jpeg;base64,{b64}"));
                }
            }
        }
    }
}

/// Generate a self-contained HTML scan report (consumer QA dashboard).
///
/// The output is a single HTML file with all CSS and JS inlined — no external
/// dependencies.  If `diff` is provided, a comparison section is included.
pub fn generate_scan_report(scan: &ScanResult, diff: Option<&ScanDiff>) -> String {
    let health = scan.health_score;
    let gauge_color = gauge_color(health);

    // Build narrative summary using the narrative module
    let flow_summaries: Vec<crate::narrative::FlowResultSummary> = scan
        .flows
        .iter()
        .map(|f| crate::narrative::FlowResultSummary {
            name: f.name.clone(),
            flow_type: parse_flow_type(&f.flow_type),
            importance: parse_importance(&f.importance),
            passed: f.passed,
            duration_ms: f.duration_ms,
            url: f.url.clone(),
        })
        .collect();

    let summary_text = crate::narrative::generate_summary(&flow_summaries);
    let speed_text = crate::narrative::speed_callout(scan.scan_duration_ms);

    // Sort flows: failures first, then by importance (critical > high > medium > low)
    let mut sorted_flows: Vec<&FlowResult> = scan.flows.iter().collect();
    sorted_flows.sort_by(|a, b| {
        // Failures first
        let fail_order = a.passed.cmp(&b.passed);
        if fail_order != std::cmp::Ordering::Equal {
            return fail_order;
        }
        // Then by importance descending
        importance_rank(&b.importance).cmp(&importance_rank(&a.importance))
    });

    // Build flow cards
    let mut flow_cards_html = String::new();
    for flow in &sorted_flows {
        flow_cards_html.push_str(&render_flow_card(flow));
    }

    // Build diff section
    let diff_html = if let Some(d) = diff {
        render_diff_section(d, scan.health_score)
    } else {
        String::new()
    };

    // Build health change indicator
    let health_change_html = if let Some(d) = diff {
        let prev = (health as i32 - d.health_score_change) as u32;
        if d.health_score_change > 0 {
            format!(r#"<div class="health-change improving">{prev} &rarr; {health}</div>"#,)
        } else if d.health_score_change < 0 {
            format!(r#"<div class="health-change regressing">{prev} &rarr; {health}</div>"#,)
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // Console errors section
    let console_errors_html = if scan.console_errors > 0 {
        format!(
            r#"<section class="console-errors">
  <div class="console-errors-icon">&#9888;</div>
  <div class="console-errors-text">
    <strong>{count} console error{s}</strong> detected during scan.
  </div>
</section>"#,
            count = scan.console_errors,
            s = if scan.console_errors == 1 { "" } else { "s" },
        )
    } else {
        String::new()
    };

    // SVG gauge — CSS-only circular gauge via stroke-dasharray
    let circumference: f64 = 2.0 * std::f64::consts::PI * 54.0; // r=54
    let dash_len = format!("{:.1}", circumference * (health as f64 / 100.0));
    let dash_gap = format!(
        "{:.1}",
        circumference - circumference * (health as f64 / 100.0)
    );

    // Badge embed code
    let badge_svg = crate::badge::generate_badge(health, "QA Health");
    // Encode the SVG for use in an <img> src via data URI
    let badge_data_uri = format!("data:image/svg+xml;base64,{}", {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(badge_svg.as_bytes())
    });
    let badge_markdown = format!("![QA Health: {health}%](YOUR_BADGE_URL)");
    let badge_img_tag = format!(
        r#"<img src="{}" alt="QA Health: {}%">"#,
        scan_html_escape(&badge_data_uri),
        health,
    );

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>QA Report &mdash; {url}</title>
<style>
*, *::before, *::after {{ box-sizing: border-box; }}
body {{
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
  margin: 0; padding: 0;
  background: #fafafa; color: #1a1a1a;
  line-height: 1.6;
}}
.container {{ max-width: 800px; margin: 0 auto; padding: 2rem 1.5rem; }}

/* Header */
.header {{
  text-align: center;
  padding: 2rem 0 1.5rem;
  border-bottom: 1px solid #e5e5e5;
  margin-bottom: 2rem;
}}
.header-url {{
  font-size: 1.1rem; color: #666;
  margin-bottom: 1.5rem;
  word-break: break-all;
}}
.header-url a {{ color: #2563eb; text-decoration: none; }}
.header-url a:hover {{ text-decoration: underline; }}
.gauge-wrap {{ display: inline-block; position: relative; width: 140px; height: 140px; }}
.gauge-svg {{ transform: rotate(-90deg); }}
.gauge-bg {{ fill: none; stroke: #e5e5e5; stroke-width: 8; }}
.gauge-fg {{ fill: none; stroke: {gauge_color}; stroke-width: 8; stroke-linecap: round; transition: stroke-dasharray 0.6s ease; }}
.gauge-label {{
  position: absolute; top: 50%; left: 50%;
  transform: translate(-50%, -50%);
  font-size: 2.2rem; font-weight: 700; color: {gauge_color};
}}
.health-change {{
  font-size: 0.95rem; font-weight: 600; margin-top: 0.5rem;
}}
.health-change.improving {{ color: #22c55e; }}
.health-change.regressing {{ color: #ef4444; }}
.header-meta {{
  font-size: 0.85rem; color: #999; margin-top: 0.75rem;
}}

/* Narrative */
.narrative {{
  background: #fff; border: 1px solid #e5e5e5; border-radius: 10px;
  padding: 1.25rem 1.5rem; margin-bottom: 1.5rem;
}}
.narrative-summary {{ font-size: 1rem; color: #333; margin-bottom: 0.5rem; }}
.narrative-speed {{ font-size: 0.85rem; color: #888; }}

/* Flow cards */
.flow-card {{
  background: #fff; border: 1px solid #e5e5e5; border-radius: 10px;
  padding: 1.25rem 1.5rem; margin-bottom: 1rem;
  box-shadow: 0 1px 3px rgba(0,0,0,0.04);
}}
.flow-card.failed {{ border-left: 4px solid #ef4444; }}
.flow-card.passed {{ border-left: 4px solid #22c55e; }}
.flow-header {{
  display: flex; align-items: center; gap: 0.6rem;
  flex-wrap: wrap; margin-bottom: 0.5rem;
}}
.flow-icon {{ font-size: 1.2rem; flex-shrink: 0; }}
.flow-icon.pass {{ color: #22c55e; }}
.flow-icon.fail {{ color: #ef4444; }}
.flow-name {{ font-weight: 600; font-size: 1rem; flex: 1; min-width: 0; }}
.badge {{
  display: inline-block; padding: 2px 8px; border-radius: 12px;
  font-size: 0.7rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.03em;
  flex-shrink: 0;
}}
.badge-critical {{ background: #fef2f2; color: #ef4444; border: 1px solid #fecaca; }}
.badge-high {{ background: #fff7ed; color: #ea580c; border: 1px solid #fed7aa; }}
.badge-medium {{ background: #eff6ff; color: #2563eb; border: 1px solid #bfdbfe; }}
.badge-low {{ background: #f9fafb; color: #6b7280; border: 1px solid #e5e7eb; }}
.flow-desc {{ font-size: 0.9rem; color: #555; margin-bottom: 0.4rem; }}
.flow-duration {{ font-size: 0.8rem; color: #999; }}
.flow-error {{
  margin-top: 0.75rem; padding: 0.6rem 0.8rem;
  background: #fef2f2; border: 1px solid #fecaca; border-radius: 6px;
  color: #b91c1c; font-size: 0.85rem;
  word-break: break-word;
}}

/* Screenshot filmstrip */
.filmstrip {{
  display: flex; gap: 8px; overflow-x: auto;
  margin-top: 0.75rem; padding: 4px 0;
}}
.filmstrip img {{
  width: 200px; height: auto; border-radius: 6px;
  border: 1px solid #e5e5e5; flex-shrink: 0;
  object-fit: cover;
}}

/* Diff / comparison section */
.diff-section {{
  background: #fff; border: 1px solid #e5e5e5; border-radius: 10px;
  padding: 1.25rem 1.5rem; margin-bottom: 1.5rem;
}}
.diff-section h2 {{ font-size: 1.1rem; margin: 0 0 0.75rem; }}
.diff-list {{ list-style: none; padding: 0; margin: 0; }}
.diff-list li {{
  padding: 0.35rem 0; font-size: 0.9rem;
  border-bottom: 1px solid #f0f0f0;
}}
.diff-list li:last-child {{ border-bottom: none; }}
.diff-regression {{ color: #ef4444; }}
.diff-improvement {{ color: #22c55e; }}
.diff-new {{ color: #2563eb; }}
.diff-removed {{ color: #999; }}
.diff-summary-text {{ font-size: 0.9rem; color: #555; margin-bottom: 0.75rem; }}

/* Console errors */
.console-errors {{
  display: flex; align-items: center; gap: 0.75rem;
  background: #fffbeb; border: 1px solid #fde68a; border-radius: 10px;
  padding: 1rem 1.25rem; margin-bottom: 1.5rem;
}}
.console-errors-icon {{ font-size: 1.5rem; color: #eab308; flex-shrink: 0; }}
.console-errors-text {{ font-size: 0.9rem; color: #92400e; }}

/* Footer */
.footer {{
  border-top: 1px solid #e5e5e5;
  padding-top: 1.5rem; margin-top: 2rem;
  text-align: center;
  font-size: 0.8rem; color: #999;
}}
.footer-speed {{ margin-top: 0.25rem; }}
.badge-embed {{
  margin-top: 1rem; padding: 0.75rem;
  background: #f5f5f5; border-radius: 8px;
  text-align: left;
}}
.badge-embed summary {{
  cursor: pointer; font-size: 0.85rem; color: #666;
  user-select: none;
}}
.badge-embed pre {{
  margin: 0.5rem 0 0; font-size: 0.75rem;
  background: #fff; padding: 0.5rem;
  border-radius: 4px; border: 1px solid #ddd;
  overflow-x: auto; white-space: pre-wrap;
  word-break: break-all;
  position: relative;
}}
.copy-btn {{
  position: absolute; top: 4px; right: 4px;
  padding: 2px 8px; font-size: 0.7rem;
  background: #e5e5e5; border: none; border-radius: 4px;
  cursor: pointer; color: #555;
}}
.copy-btn:hover {{ background: #d5d5d5; }}

/* Responsive */
@media (max-width: 600px) {{
  .container {{ padding: 1rem; }}
  .gauge-wrap {{ width: 110px; height: 110px; }}
  .gauge-label {{ font-size: 1.8rem; }}
  .filmstrip img {{ width: 150px; }}
}}
</style>
</head>
<body>
<div class="container">

<!-- Header -->
<header class="header">
  <div class="header-url"><a href="{url_raw}">{url}</a></div>
  <div class="gauge-wrap">
    <svg class="gauge-svg" viewBox="0 0 120 120" width="100%" height="100%">
      <circle class="gauge-bg" cx="60" cy="60" r="54"/>
      <circle class="gauge-fg" cx="60" cy="60" r="54"
        stroke-dasharray="{dash_len} {dash_gap}"/>
    </svg>
    <span class="gauge-label">{health}</span>
  </div>
  {health_change_html}
  <div class="header-meta">{timestamp} &middot; {framework}</div>
</header>

<!-- Narrative summary -->
<section class="narrative">
  <div class="narrative-summary">{summary_text}</div>
  <div class="narrative-speed">{speed_text}</div>
</section>

<!-- Console errors -->
{console_errors_html}

<!-- Comparison -->
{diff_html}

<!-- Flow cards -->
{flow_cards_html}

<!-- Footer -->
<footer class="footer">
  <div>Scanned by <strong>rayo</strong> &middot; {timestamp}</div>
  <div class="footer-speed">{speed_text_footer}</div>
  <div class="badge-embed">
    <details>
      <summary>Embed health badge</summary>
      <div style="margin-top:0.5rem;text-align:center;">
        <img src="{badge_data_uri}" alt="QA Health: {health}%">
      </div>
      <p style="font-size:0.8rem;color:#888;margin:0.5rem 0 0.25rem;">Markdown</p>
      <pre id="badge-md">{badge_md}<button class="copy-btn" onclick="copyBadge('badge-md')">Copy</button></pre>
      <p style="font-size:0.8rem;color:#888;margin:0.5rem 0 0.25rem;">HTML</p>
      <pre id="badge-html">{badge_html}<button class="copy-btn" onclick="copyBadge('badge-html')">Copy</button></pre>
    </details>
  </div>
</footer>

</div>
<script>
function copyBadge(id){{
  var el=document.getElementById(id);
  // Get text content excluding the button
  var text='';
  for(var i=0;i<el.childNodes.length;i++){{
    if(el.childNodes[i].nodeType===3) text+=el.childNodes[i].textContent;
  }}
  navigator.clipboard.writeText(text.trim()).then(function(){{
    var btn=el.querySelector('.copy-btn');
    btn.textContent='Copied!';
    setTimeout(function(){{btn.textContent='Copy';}},1500);
  }});
}}
</script>
</body>
</html>"##,
        url = scan_html_escape(&scan.url),
        url_raw = scan_html_escape(&scan.url),
        health = health,
        gauge_color = gauge_color,
        dash_len = dash_len,
        dash_gap = dash_gap,
        health_change_html = health_change_html,
        timestamp = scan_html_escape(&scan.timestamp),
        framework = scan_html_escape(&scan.framework),
        summary_text = summary_text, // already html-escaped by narrative module
        speed_text = speed_text,     // already html-escaped by narrative module
        speed_text_footer = speed_text,
        console_errors_html = console_errors_html,
        diff_html = diff_html,
        flow_cards_html = flow_cards_html,
        badge_data_uri = scan_html_escape(&badge_data_uri),
        badge_md = scan_html_escape(&badge_markdown),
        badge_html = scan_html_escape(&badge_img_tag),
    )
}

/// Render a single flow card as HTML.
fn render_flow_card(flow: &FlowResult) -> String {
    let status_class = if flow.passed { "passed" } else { "failed" };
    let icon_class = if flow.passed { "pass" } else { "fail" };
    let icon_char = if flow.passed { "&#10003;" } else { "&#10007;" };
    let badge_class = match flow.importance.as_str() {
        "critical" => "badge-critical",
        "high" => "badge-high",
        "medium" => "badge-medium",
        _ => "badge-low",
    };
    let importance_label = scan_html_escape(&capitalize(&flow.importance));

    // Error block
    let error_html = if let Some(ref err) = flow.error {
        format!(r#"<div class="flow-error">{}</div>"#, scan_html_escape(err))
    } else {
        String::new()
    };

    // Screenshot filmstrip — collect up to 5 data-URI screenshots
    let screenshots: Vec<&str> = flow
        .steps
        .iter()
        .filter_map(|s| s.screenshot_path.as_deref())
        .filter(|p| p.starts_with("data:"))
        .take(5)
        .collect();

    let filmstrip_html = if screenshots.is_empty() {
        String::new()
    } else {
        let imgs: String = screenshots
            .iter()
            .map(|src| {
                format!(
                    r#"<img src="{}" alt="Step screenshot" loading="lazy">"#,
                    scan_html_escape(src)
                )
            })
            .collect::<Vec<_>>()
            .join("\n    ");
        format!(r#"<div class="filmstrip">{imgs}</div>"#)
    };

    let duration_display = format_duration_display(flow.duration_ms);

    format!(
        r#"<div class="flow-card {status_class}">
  <div class="flow-header">
    <span class="flow-icon {icon_class}">{icon_char}</span>
    <span class="flow-name">{name}</span>
    <span class="badge {badge_class}">{importance_label}</span>
  </div>
  <div class="flow-desc">{description}</div>
  <div class="flow-duration">{duration}</div>
  {error_html}
  {filmstrip_html}
</div>
"#,
        name = scan_html_escape(&flow.name),
        description = scan_html_escape(&flow.description),
        duration = scan_html_escape(&duration_display),
        error_html = error_html,
        filmstrip_html = filmstrip_html,
    )
}

/// Render the diff / comparison section.
fn render_diff_section(diff: &ScanDiff, current_health: u32) -> String {
    let prev_health = (current_health as i32 - diff.health_score_change) as u32;
    let summary_text = crate::rundiff::diff_summary_with_scores(diff, prev_health, current_health);

    let mut items = String::new();

    for f in &diff.flows_regressed {
        items.push_str(&format!(
            r#"<li class="diff-regression">&#10007; {} &mdash; was passing, now failing</li>"#,
            scan_html_escape(&f.name)
        ));
    }
    for f in &diff.flows_improved {
        items.push_str(&format!(
            r#"<li class="diff-improvement">&#10003; {} &mdash; was failing, now passing</li>"#,
            scan_html_escape(&f.name)
        ));
    }
    for name in &diff.flows_new {
        items.push_str(&format!(
            r#"<li class="diff-new">+ {} (new)</li>"#,
            scan_html_escape(name)
        ));
    }
    for name in &diff.flows_removed {
        items.push_str(&format!(
            r#"<li class="diff-removed">&minus; {} (removed)</li>"#,
            scan_html_escape(name)
        ));
    }

    format!(
        r#"<section class="diff-section">
  <h2>Compared to previous scan</h2>
  <div class="diff-summary-text">{summary}</div>
  <ul class="diff-list">
    {items}
  </ul>
</section>"#,
        summary = scan_html_escape(&summary_text),
        items = items,
    )
}

/// Return the gauge color hex string for a given health score.
fn gauge_color(score: u32) -> &'static str {
    if score >= 80 {
        "#22c55e"
    } else if score >= 50 {
        "#eab308"
    } else {
        "#ef4444"
    }
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

/// Capitalize the first character of a string.
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => {
            let upper: String = first.to_uppercase().collect();
            format!("{}{}", upper, chars.as_str())
        }
        None => String::new(),
    }
}

/// Format a duration for display.
fn format_duration_display(ms: u64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else {
        let secs = ms as f64 / 1000.0;
        format!("{:.1}s", secs)
    }
}

/// Parse a flow type string into a [`FlowType`] for narrative functions.
fn parse_flow_type(s: &str) -> crate::discover::flows::FlowType {
    match s {
        "auth" => crate::discover::flows::FlowType::Auth,
        "form" => crate::discover::flows::FlowType::Form,
        "crud" => crate::discover::flows::FlowType::Crud,
        "search" => crate::discover::flows::FlowType::Search,
        _ => crate::discover::flows::FlowType::Navigation,
    }
}

/// Parse an importance string into a [`FlowImportance`] for narrative functions.
fn parse_importance(s: &str) -> crate::narrative::FlowImportance {
    match s {
        "critical" => crate::narrative::FlowImportance::Critical,
        "high" => crate::narrative::FlowImportance::High,
        "medium" => crate::narrative::FlowImportance::Medium,
        _ => crate::narrative::FlowImportance::Low,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::{FlowResult, FlowStepResult, ScanResult};
    use crate::result::{AssertionResult, StepResult, SuiteResult};
    use crate::rundiff::{FlowDiff, ScanDiff};

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

    // -----------------------------------------------------------------------
    // Scan report tests
    // -----------------------------------------------------------------------

    fn sample_scan(health_score: u32, console_errors: u32) -> ScanResult {
        ScanResult {
            url: "http://localhost:3000".to_string(),
            framework: "Next.js".to_string(),
            health_score,
            total_flows: 3,
            passed_flows: 2,
            failed_flows: 1,
            total_duration_ms: 2500,
            scan_duration_ms: 2200,
            console_errors,
            timestamp: "2026-03-21T12:00:00Z".to_string(),
            flows: vec![
                FlowResult {
                    name: "Login Flow".to_string(),
                    description: "Tests user login with valid credentials".to_string(),
                    flow_type: "auth".to_string(),
                    importance: "critical".to_string(),
                    url: "http://localhost:3000/login".to_string(),
                    passed: true,
                    duration_ms: 800,
                    steps: vec![
                        FlowStepResult {
                            name: "Navigate to login".to_string(),
                            action: "navigate".to_string(),
                            passed: true,
                            duration_ms: 300,
                            error: None,
                            screenshot_path: None,
                        },
                        FlowStepResult {
                            name: "Fill credentials".to_string(),
                            action: "type".to_string(),
                            passed: true,
                            duration_ms: 200,
                            error: None,
                            screenshot_path: None,
                        },
                    ],
                    error: None,
                },
                FlowResult {
                    name: "Search".to_string(),
                    description: "Tests search functionality".to_string(),
                    flow_type: "search".to_string(),
                    importance: "medium".to_string(),
                    url: "http://localhost:3000/search".to_string(),
                    passed: false,
                    duration_ms: 1200,
                    steps: vec![],
                    error: Some("Element not found: #search-input".to_string()),
                },
                FlowResult {
                    name: "Home Navigation".to_string(),
                    description: "Visits homepage and checks links".to_string(),
                    flow_type: "navigation".to_string(),
                    importance: "low".to_string(),
                    url: "http://localhost:3000/".to_string(),
                    passed: true,
                    duration_ms: 500,
                    steps: vec![],
                    error: None,
                },
            ],
        }
    }

    #[test]
    fn scan_report_contains_url() {
        let scan = sample_scan(85, 0);
        let html = generate_scan_report(&scan, None);
        assert!(html.contains("http://localhost:3000"), "URL should appear");
    }

    #[test]
    fn scan_report_contains_health_gauge() {
        let scan = sample_scan(85, 0);
        let html = generate_scan_report(&scan, None);
        assert!(html.contains("gauge-fg"), "should have SVG gauge");
        assert!(
            html.contains("stroke-dasharray"),
            "should use stroke-dasharray"
        );
        assert!(html.contains(">85<"), "should show health score number");
    }

    #[test]
    fn scan_report_green_gauge_for_high_score() {
        let scan = sample_scan(92, 0);
        let html = generate_scan_report(&scan, None);
        assert!(html.contains("#22c55e"), ">=80 should be green");
    }

    #[test]
    fn scan_report_yellow_gauge_for_mid_score() {
        let scan = sample_scan(65, 0);
        let html = generate_scan_report(&scan, None);
        assert!(html.contains("#eab308"), ">=50 should be yellow");
    }

    #[test]
    fn scan_report_red_gauge_for_low_score() {
        let scan = sample_scan(30, 0);
        let html = generate_scan_report(&scan, None);
        assert!(html.contains("#ef4444"), "<50 should be red");
    }

    #[test]
    fn scan_report_contains_narrative() {
        let scan = sample_scan(85, 0);
        let html = generate_scan_report(&scan, None);
        assert!(
            html.contains("narrative-summary"),
            "should have narrative section"
        );
        assert!(
            html.contains("narrative-speed"),
            "should have speed callout"
        );
        assert!(
            html.contains("Scanned in"),
            "speed callout should mention scan time"
        );
    }

    #[test]
    fn scan_report_contains_flow_cards() {
        let scan = sample_scan(85, 0);
        let html = generate_scan_report(&scan, None);
        assert!(html.contains("Login Flow"), "should show Login Flow");
        assert!(html.contains("Search"), "should show Search flow");
        assert!(
            html.contains("Home Navigation"),
            "should show Home Navigation"
        );
    }

    #[test]
    fn scan_report_failures_first() {
        let scan = sample_scan(85, 0);
        let html = generate_scan_report(&scan, None);
        let search_pos = html.find("Search").unwrap();
        let login_pos = html.find("Login Flow").unwrap();
        // Search is failing so it should appear before Login
        assert!(
            search_pos < login_pos,
            "failed flows should appear before passing flows"
        );
    }

    #[test]
    fn scan_report_shows_error_message() {
        let scan = sample_scan(85, 0);
        let html = generate_scan_report(&scan, None);
        assert!(
            html.contains("Element not found: #search-input"),
            "should show error message"
        );
        assert!(
            html.contains("flow-error"),
            "error should use flow-error class"
        );
    }

    #[test]
    fn scan_report_importance_badges() {
        let scan = sample_scan(85, 0);
        let html = generate_scan_report(&scan, None);
        assert!(
            html.contains("badge-critical"),
            "should have critical badge"
        );
        assert!(html.contains("badge-medium"), "should have medium badge");
        assert!(html.contains("badge-low"), "should have low badge");
    }

    #[test]
    fn scan_report_console_errors() {
        let scan = sample_scan(85, 3);
        let html = generate_scan_report(&scan, None);
        assert!(
            html.contains("3 console errors"),
            "should show console error count"
        );
        assert!(
            html.contains("console-errors"),
            "should use console-errors class"
        );
    }

    #[test]
    fn scan_report_no_console_errors_section_when_zero() {
        let scan = sample_scan(85, 0);
        let html = generate_scan_report(&scan, None);
        // The CSS class names appear in the <style> block, but the actual
        // section element with console error count should not be rendered.
        assert!(
            !html.contains("console error"),
            "should not show console errors section when 0"
        );
    }

    #[test]
    fn scan_report_footer() {
        let scan = sample_scan(85, 0);
        let html = generate_scan_report(&scan, None);
        assert!(html.contains("Scanned by"), "should have footer");
        assert!(html.contains("rayo"), "footer should mention rayo");
        assert!(
            html.contains("badge-embed"),
            "should have badge embed section"
        );
    }

    #[test]
    fn scan_report_self_contained() {
        let scan = sample_scan(85, 0);
        let html = generate_scan_report(&scan, None);
        assert!(html.contains("<style>"), "should have inline CSS");
        assert!(html.contains("<script>"), "should have inline JS");
        assert!(
            !html.contains("https://cdn"),
            "should not reference any CDN"
        );
        // No external stylesheets (rel="stylesheet" with http href)
        assert!(
            !html.contains("rel=\"stylesheet\""),
            "should not have external stylesheet links"
        );
    }

    #[test]
    fn scan_report_html_escapes_content() {
        let mut scan = sample_scan(85, 0);
        scan.url = "http://example.com/<script>".to_string();
        scan.flows[0].name = "Test <b>bold</b>".to_string();
        scan.flows[1].error = Some("Error: <script>alert(1)</script>".to_string());

        let html = generate_scan_report(&scan, None);
        assert!(!html.contains("<script>alert"), "XSS should be escaped");
        assert!(
            html.contains("&lt;script&gt;alert"),
            "should contain escaped script tag"
        );
    }

    #[test]
    fn scan_report_with_diff() {
        let scan = sample_scan(85, 0);
        let diff = ScanDiff {
            previous_timestamp: "2026-03-20T12:00:00Z".to_string(),
            current_timestamp: "2026-03-21T12:00:00Z".to_string(),
            health_score_change: 10,
            flows_regressed: vec![],
            flows_improved: vec![FlowDiff {
                name: "Search".to_string(),
                flow_type: "search".to_string(),
                previous_passed: false,
                current_passed: true,
                duration_change_ms: -200,
            }],
            flows_unchanged: vec![],
            flows_new: vec!["Dashboard".to_string()],
            flows_removed: vec!["Legacy".to_string()],
        };

        let html = generate_scan_report(&scan, Some(&diff));
        assert!(
            html.contains("Compared to previous scan"),
            "should have diff section"
        );
        assert!(
            html.contains("diff-improvement"),
            "should show improvements"
        );
        assert!(html.contains("diff-new"), "should show new flows");
        assert!(html.contains("diff-removed"), "should show removed flows");
        assert!(
            html.contains("health-change"),
            "should show health change indicator"
        );
    }

    #[test]
    fn scan_report_no_diff_section_without_diff() {
        let scan = sample_scan(85, 0);
        let html = generate_scan_report(&scan, None);
        assert!(
            !html.contains("Compared to previous scan"),
            "should not have diff section without diff"
        );
    }

    #[test]
    fn scan_report_with_regression_diff() {
        let scan = sample_scan(70, 0);
        let diff = ScanDiff {
            previous_timestamp: "2026-03-20T12:00:00Z".to_string(),
            current_timestamp: "2026-03-21T12:00:00Z".to_string(),
            health_score_change: -15,
            flows_regressed: vec![FlowDiff {
                name: "Login".to_string(),
                flow_type: "auth".to_string(),
                previous_passed: true,
                current_passed: false,
                duration_change_ms: 300,
            }],
            flows_improved: vec![],
            flows_unchanged: vec![],
            flows_new: vec![],
            flows_removed: vec![],
        };

        let html = generate_scan_report(&scan, Some(&diff));
        assert!(
            html.contains("regressing"),
            "should show regressing indicator"
        );
        assert!(
            html.contains("diff-regression"),
            "should show regression items"
        );
    }

    #[test]
    fn inline_screenshots_replaces_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let screenshots_dir = tmp.path().join("screenshots");
        std::fs::create_dir_all(&screenshots_dir).unwrap();

        // Write a tiny fake JPEG
        let fake_jpeg = b"\xFF\xD8\xFF\xE0test";
        std::fs::write(screenshots_dir.join("login-step0.jpg"), fake_jpeg).unwrap();

        let mut scan = sample_scan(85, 0);
        scan.flows[0].steps[0].screenshot_path = Some("screenshots/login-step0.jpg".to_string());

        inline_screenshots(&mut scan, tmp.path());

        let path = scan.flows[0].steps[0].screenshot_path.as_ref().unwrap();
        assert!(
            path.starts_with("data:image/jpeg;base64,"),
            "should be a data URI: {}",
            &path[..40.min(path.len())]
        );
    }

    #[test]
    fn inline_screenshots_skips_missing_files() {
        let tmp = tempfile::tempdir().unwrap();
        let mut scan = sample_scan(85, 0);
        scan.flows[0].steps[0].screenshot_path = Some("screenshots/nonexistent.jpg".to_string());

        inline_screenshots(&mut scan, tmp.path());

        // Path should be unchanged
        assert_eq!(
            scan.flows[0].steps[0].screenshot_path.as_deref(),
            Some("screenshots/nonexistent.jpg"),
        );
    }

    #[test]
    fn inline_screenshots_skips_already_inlined() {
        let tmp = tempfile::tempdir().unwrap();
        let mut scan = sample_scan(85, 0);
        let data_uri = "data:image/jpeg;base64,AAAA";
        scan.flows[0].steps[0].screenshot_path = Some(data_uri.to_string());

        inline_screenshots(&mut scan, tmp.path());

        assert_eq!(
            scan.flows[0].steps[0].screenshot_path.as_deref(),
            Some(data_uri),
        );
    }

    #[test]
    fn scan_report_filmstrip_with_data_uris() {
        let mut scan = sample_scan(85, 0);
        scan.flows[0].steps[0].screenshot_path = Some("data:image/jpeg;base64,AAAA".to_string());

        let html = generate_scan_report(&scan, None);
        assert!(
            html.contains("filmstrip"),
            "should have filmstrip when screenshots exist"
        );
        assert!(
            html.contains("data:image/jpeg;base64,AAAA"),
            "should inline the data URI"
        );
    }

    #[test]
    fn gauge_color_thresholds() {
        assert_eq!(gauge_color(100), "#22c55e");
        assert_eq!(gauge_color(80), "#22c55e");
        assert_eq!(gauge_color(79), "#eab308");
        assert_eq!(gauge_color(50), "#eab308");
        assert_eq!(gauge_color(49), "#ef4444");
        assert_eq!(gauge_color(0), "#ef4444");
    }

    #[test]
    fn importance_rank_ordering() {
        assert!(importance_rank("critical") > importance_rank("high"));
        assert!(importance_rank("high") > importance_rank("medium"));
        assert!(importance_rank("medium") > importance_rank("low"));
        assert!(importance_rank("low") > importance_rank("unknown"));
    }

    #[test]
    fn capitalize_works() {
        assert_eq!(capitalize("critical"), "Critical");
        assert_eq!(capitalize("high"), "High");
        assert_eq!(capitalize(""), "");
    }

    #[test]
    fn format_duration_display_works() {
        assert_eq!(format_duration_display(500), "500ms");
        assert_eq!(format_duration_display(999), "999ms");
        assert_eq!(format_duration_display(1000), "1.0s");
        assert_eq!(format_duration_display(2500), "2.5s");
    }

    #[test]
    fn scan_html_escape_works() {
        assert_eq!(scan_html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(scan_html_escape("a & b"), "a &amp; b");
        assert_eq!(scan_html_escape("it's"), "it&#x27;s");
        assert_eq!(
            scan_html_escape(r#"he said "hi""#),
            "he said &quot;hi&quot;"
        );
    }

    #[test]
    fn scan_report_responsive_meta_viewport() {
        let scan = sample_scan(85, 0);
        let html = generate_scan_report(&scan, None);
        assert!(
            html.contains("viewport"),
            "should have viewport meta for responsive"
        );
    }

    #[test]
    fn scan_report_copy_badge_js() {
        let scan = sample_scan(85, 0);
        let html = generate_scan_report(&scan, None);
        assert!(
            html.contains("copyBadge"),
            "should have copy badge JS function"
        );
        assert!(
            html.contains("navigator.clipboard"),
            "should use clipboard API"
        );
    }
}
