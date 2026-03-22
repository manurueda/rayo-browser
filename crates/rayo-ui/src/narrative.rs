//! Narrative descriptions for detected user flows.
//!
//! Pure-function module that transforms flow detection data into
//! human-readable English descriptions and summaries. No LLM — just
//! template-based string formatting.

use crate::discover::flows::{DetectedFlow, FlowStep, FlowType};

/// Importance level for a user flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FlowImportance {
    Low,
    Medium,
    High,
    Critical,
}

impl FlowImportance {
    pub fn as_str(&self) -> &str {
        match self {
            FlowImportance::Critical => "critical",
            FlowImportance::High => "high",
            FlowImportance::Medium => "medium",
            FlowImportance::Low => "low",
        }
    }
}

impl std::fmt::Display for FlowImportance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Summary of a flow's test result, used to generate narrative summaries.
#[derive(Debug, Clone)]
pub struct FlowResultSummary {
    pub name: String,
    pub flow_type: FlowType,
    pub importance: FlowImportance,
    pub passed: bool,
    pub duration_ms: u64,
    pub url: String,
}

/// Determine the importance of a flow based on its type and name.
pub fn flow_importance(flow_type: &FlowType, name: &str) -> FlowImportance {
    let name_lower = name.to_lowercase();
    match flow_type {
        FlowType::Auth => FlowImportance::Critical,
        FlowType::Form => {
            if name_lower.contains("checkout")
                || name_lower.contains("signup")
                || name_lower.contains("payment")
            {
                FlowImportance::High
            } else {
                FlowImportance::Medium
            }
        }
        FlowType::Crud => FlowImportance::Medium,
        FlowType::Search => FlowImportance::Medium,
        FlowType::Navigation => FlowImportance::Low,
    }
}

/// Generate a one-liner description of a detected flow.
pub fn describe_flow(flow: &DetectedFlow) -> String {
    let url_path = extract_url_path(&flow.url);
    let page_name = page_name_from_path(&url_path);

    match flow.flow_type {
        FlowType::Auth => {
            let field_names = extract_field_names(&flow.steps);
            let submit_text = extract_submit_text(&flow.steps);
            format!(
                "Login: visits {}, enters {}, clicks {}",
                html_escape(&url_path),
                html_escape(&field_names),
                html_escape(&submit_text),
            )
        }
        FlowType::Search => {
            let query = extract_search_query(&flow.steps);
            format!(
                "Search: visits {}, searches for '{}', verifies results",
                html_escape(&url_path),
                html_escape(&query),
            )
        }
        FlowType::Form => {
            let field_names = extract_field_names(&flow.steps);
            format!(
                "{} Form: fills out {}, submits form",
                html_escape(&page_name),
                html_escape(&field_names),
            )
        }
        FlowType::Crud => {
            let field_names = extract_field_names(&flow.steps);
            format!(
                "{} Data: creates entry with {}, verifies list updates",
                html_escape(&page_name),
                html_escape(&field_names),
            )
        }
        FlowType::Navigation => {
            let link_count = flow.steps.iter().filter(|s| s.action == "click").count();
            format!(
                "Site Navigation: visits {}, clicks {} internal links, all pages load",
                html_escape(&url_path),
                link_count,
            )
        }
    }
}

/// Generate a narrative summary paragraph for a set of flow results.
pub fn generate_summary(results: &[FlowResultSummary]) -> String {
    if results.is_empty() {
        return "No flows were tested.".to_string();
    }

    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let failed = total - passed;

    if passed == total {
        // All pass
        let top_flow = results
            .iter()
            .max_by_key(|r| r.importance)
            .expect("non-empty results");
        let speed = speed_judgment(top_flow.duration_ms);
        let timing = format_duration(top_flow.duration_ms);
        format!(
            "Your app is healthy. All {} user flows work correctly. {} takes {} ({}). No critical issues detected.",
            total,
            html_escape(&top_flow.name),
            timing,
            speed,
        )
    } else if failed == total {
        // All fail
        format!(
            "Your app has critical issues. All {} tested flows are failing.",
            total,
        )
    } else {
        // Some fail
        let critical_failures: Vec<&FlowResultSummary> = results
            .iter()
            .filter(|r| !r.passed && r.importance == FlowImportance::Critical)
            .collect();

        let critical_sentence = if critical_failures.is_empty() {
            String::new()
        } else {
            let names: Vec<&str> = critical_failures.iter().map(|r| r.name.as_str()).collect();
            format!(" Critical failure: {}.", html_escape(&names.join(", ")),)
        };

        let broken: Vec<&str> = results
            .iter()
            .filter(|r| !r.passed)
            .map(|r| r.name.as_str())
            .collect();
        let broken_list = html_escape(&broken.join(", "));

        format!(
            "Your app needs attention. {}/{} flows pass.{} Broken: {}.",
            passed, total, critical_sentence, broken_list,
        )
    }
}

/// Generate a speed comparison callout string.
pub fn speed_callout(total_duration_ms: u64) -> String {
    let duration = format_duration(total_duration_ms);
    format!(
        "Scanned in {}. Typical Playwright suite: ~45s. Manual QA: ~15 min.",
        duration,
    )
}

/// HTML-escape a string to prevent XSS when embedding in HTML output.
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Extract the path component from a URL.
fn extract_url_path(url: &str) -> String {
    if let Some(idx) = url.find("://") {
        let after_scheme = &url[idx + 3..];
        if let Some(slash_idx) = after_scheme.find('/') {
            after_scheme[slash_idx..].to_string()
        } else {
            "/".to_string()
        }
    } else {
        url.to_string()
    }
}

/// Derive a human-readable page name from a URL path (e.g. "/contact" -> "Contact").
fn page_name_from_path(path: &str) -> String {
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        return "Home".to_string();
    }
    // Take the last segment
    let segment = trimmed.rsplit('/').next().unwrap_or(trimmed);
    // Title-case: first char upper, rest lower, replace hyphens/underscores with spaces
    let words: Vec<String> = segment
        .split(['-', '_'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    format!("{}{}", upper, chars.as_str())
                }
                None => String::new(),
            }
        })
        .collect();
    if words.is_empty() {
        "Home".to_string()
    } else {
        words.join(" ")
    }
}

/// Extract field names from flow steps by parsing selectors like `input[name=email]`.
fn extract_field_names(steps: &[FlowStep]) -> String {
    let names: Vec<String> = steps
        .iter()
        .filter(|s| s.action == "type")
        .filter_map(|s| s.selector.as_ref())
        .map(|sel| extract_name_from_selector(sel))
        .collect();

    if names.is_empty() {
        "fields".to_string()
    } else {
        names.join(", ")
    }
}

/// Extract a field name from a CSS selector.
/// Handles patterns like `input[name=email]`, `#input-1`, `input[name="full_name"]`.
fn extract_name_from_selector(selector: &str) -> String {
    // Try to extract from [name=...] attribute
    if let Some(start) = selector.find("[name=") {
        let after = &selector[start + 6..];
        // Strip optional quotes
        let after = after.trim_start_matches(['"', '\'']);
        if let Some(end) = after.find([']', '"', '\'']) {
            return after[..end].to_string();
        }
    }

    // Try to extract from id-like patterns: #foo-1 -> "foo"
    if let Some(hash_idx) = selector.find('#') {
        let after = &selector[hash_idx + 1..];
        let id = after
            .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
            .next()
            .unwrap_or(after);
        // Strip trailing numeric id suffix (e.g. "input-1" -> "input")
        let cleaned = id.trim_end_matches(|c: char| c.is_ascii_digit() || c == '-');
        if !cleaned.is_empty() {
            return cleaned.to_string();
        }
        return id.to_string();
    }

    // Fallback: return the selector itself, trimmed
    selector.to_string()
}

/// Extract the submit button text from flow steps.
fn extract_submit_text(steps: &[FlowStep]) -> String {
    // The last click step before an assert is typically the submit
    steps
        .iter()
        .rev()
        .find(|s| s.action == "click")
        .and_then(|s| s.selector.as_ref())
        .map(|sel| {
            // Try to get a readable name from the selector
            let name = extract_name_from_selector(sel);
            if name == *sel {
                "submit".to_string()
            } else {
                name
            }
        })
        .unwrap_or_else(|| "submit".to_string())
}

/// Extract the search query from flow steps.
fn extract_search_query(steps: &[FlowStep]) -> String {
    steps
        .iter()
        .find(|s| s.action == "type")
        .and_then(|s| s.value.as_ref())
        .cloned()
        .unwrap_or_else(|| "test query".to_string())
}

/// Classify speed based on duration.
fn speed_judgment(ms: u64) -> &'static str {
    match ms {
        0..500 => "fast",
        500..2000 => "good",
        2000..5000 => "slow",
        _ => "very slow",
    }
}

/// Format a duration in ms to a human-readable string.
fn format_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else {
        let secs = ms as f64 / 1000.0;
        format!("{:.1}s", secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Helper: build a DetectedFlow with given steps
    // -----------------------------------------------------------------------
    fn make_flow(name: &str, flow_type: FlowType, url: &str, steps: Vec<FlowStep>) -> DetectedFlow {
        DetectedFlow {
            name: name.to_string(),
            flow_type,
            steps,
            url: url.to_string(),
        }
    }

    fn step(action: &str, selector: Option<&str>, value: Option<&str>) -> FlowStep {
        FlowStep {
            action: action.to_string(),
            selector: selector.map(|s| s.to_string()),
            value: value.map(|v| v.to_string()),
            url: None,
            assertion: None,
        }
    }

    fn nav_step(url: &str) -> FlowStep {
        FlowStep {
            action: "navigate".to_string(),
            selector: None,
            value: None,
            url: Some(url.to_string()),
            assertion: None,
        }
    }

    fn assert_step() -> FlowStep {
        FlowStep {
            action: "assert".to_string(),
            selector: None,
            value: None,
            url: None,
            assertion: Some("page_map_contains_any".to_string()),
        }
    }

    // -----------------------------------------------------------------------
    // flow_importance tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_importance_auth_is_critical() {
        assert_eq!(
            flow_importance(&FlowType::Auth, "Login Flow"),
            FlowImportance::Critical
        );
    }

    #[test]
    fn test_importance_form_checkout_is_high() {
        assert_eq!(
            flow_importance(&FlowType::Form, "Checkout Form"),
            FlowImportance::High
        );
    }

    #[test]
    fn test_importance_form_signup_is_high() {
        assert_eq!(
            flow_importance(&FlowType::Form, "Signup Form"),
            FlowImportance::High
        );
    }

    #[test]
    fn test_importance_form_payment_is_high() {
        assert_eq!(
            flow_importance(&FlowType::Form, "Payment Form"),
            FlowImportance::High
        );
    }

    #[test]
    fn test_importance_form_generic_is_medium() {
        assert_eq!(
            flow_importance(&FlowType::Form, "Contact Form"),
            FlowImportance::Medium
        );
    }

    #[test]
    fn test_importance_crud_is_medium() {
        assert_eq!(
            flow_importance(&FlowType::Crud, "Users CRUD"),
            FlowImportance::Medium
        );
    }

    #[test]
    fn test_importance_search_is_medium() {
        assert_eq!(
            flow_importance(&FlowType::Search, "Product Search"),
            FlowImportance::Medium
        );
    }

    #[test]
    fn test_importance_navigation_is_low() {
        assert_eq!(
            flow_importance(&FlowType::Navigation, "Site Nav"),
            FlowImportance::Low
        );
    }

    // -----------------------------------------------------------------------
    // describe_flow tests — one per FlowType
    // -----------------------------------------------------------------------

    #[test]
    fn test_describe_auth_flow() {
        let flow = make_flow(
            "Login Flow",
            FlowType::Auth,
            "http://localhost:3000/login",
            vec![
                nav_step("http://localhost:3000/login"),
                step("type", Some("input[name=email]"), Some("test@example.com")),
                step(
                    "type",
                    Some("input[name=password]"),
                    Some("TestPassword123!"),
                ),
                step("click", Some("#button-submit"), None),
                assert_step(),
            ],
        );

        let desc = describe_flow(&flow);
        assert!(desc.starts_with("Login: visits /login"), "got: {desc}");
        assert!(desc.contains("email"), "should mention email field: {desc}");
        assert!(
            desc.contains("password"),
            "should mention password field: {desc}"
        );
    }

    #[test]
    fn test_describe_search_flow() {
        let flow = make_flow(
            "Home Search",
            FlowType::Search,
            "http://localhost:3000/",
            vec![
                nav_step("http://localhost:3000/"),
                step("type", Some("input[name=q]"), Some("test query")),
                step("click", Some("#search-btn"), None),
                assert_step(),
            ],
        );

        let desc = describe_flow(&flow);
        assert!(desc.starts_with("Search: visits /"), "got: {desc}");
        assert!(
            desc.contains("test query"),
            "should mention the query: {desc}"
        );
        assert!(
            desc.contains("verifies results"),
            "should verify results: {desc}"
        );
    }

    #[test]
    fn test_describe_form_flow() {
        let flow = make_flow(
            "Contact Form",
            FlowType::Form,
            "http://localhost:3000/contact",
            vec![
                nav_step("http://localhost:3000/contact"),
                step("type", Some("input[name=name]"), Some("Test User")),
                step("type", Some("input[name=email]"), Some("test@example.com")),
                step("click", Some("button[type=submit]"), None),
                assert_step(),
            ],
        );

        let desc = describe_flow(&flow);
        assert!(
            desc.starts_with("Contact Form:"),
            "should start with page name + Form: {desc}"
        );
        assert!(desc.contains("name"), "should mention name field: {desc}");
        assert!(desc.contains("email"), "should mention email field: {desc}");
        assert!(
            desc.contains("submits form"),
            "should mention submit: {desc}"
        );
    }

    #[test]
    fn test_describe_crud_flow() {
        let flow = make_flow(
            "Users CRUD",
            FlowType::Crud,
            "http://localhost:3000/admin/users",
            vec![
                nav_step("http://localhost:3000/admin/users"),
                step("type", Some("input[name=username]"), Some("newuser")),
                step("type", Some("input[name=email]"), Some("new@example.com")),
                step("click", Some("button[type=submit]"), None),
                assert_step(),
            ],
        );

        let desc = describe_flow(&flow);
        assert!(
            desc.starts_with("Users Data:"),
            "should start with page name + Data: {desc}"
        );
        assert!(
            desc.contains("username"),
            "should mention username field: {desc}"
        );
        assert!(
            desc.contains("verifies list updates"),
            "should verify list: {desc}"
        );
    }

    #[test]
    fn test_describe_navigation_flow() {
        let flow = make_flow(
            "Home Navigation",
            FlowType::Navigation,
            "http://localhost:3000/",
            vec![
                nav_step("http://localhost:3000/"),
                step("click", Some("a[href='/about']"), None),
                assert_step(),
                step("click", Some("a[href='/contact']"), None),
                assert_step(),
                step("click", Some("a[href='/blog']"), None),
                assert_step(),
            ],
        );

        let desc = describe_flow(&flow);
        assert!(desc.starts_with("Site Navigation: visits /"), "got: {desc}");
        assert!(
            desc.contains("3 internal links"),
            "should count 3 clicks: {desc}"
        );
        assert!(
            desc.contains("all pages load"),
            "should mention pages load: {desc}"
        );
    }

    // -----------------------------------------------------------------------
    // generate_summary tests
    // -----------------------------------------------------------------------

    fn make_result(
        name: &str,
        flow_type: FlowType,
        importance: FlowImportance,
        passed: bool,
        duration_ms: u64,
    ) -> FlowResultSummary {
        FlowResultSummary {
            name: name.to_string(),
            flow_type,
            importance,
            passed,
            duration_ms,
            url: "http://localhost:3000".to_string(),
        }
    }

    #[test]
    fn test_summary_all_pass() {
        let results = vec![
            make_result("Login", FlowType::Auth, FlowImportance::Critical, true, 350),
            make_result(
                "Search",
                FlowType::Search,
                FlowImportance::Medium,
                true,
                200,
            ),
            make_result("Nav", FlowType::Navigation, FlowImportance::Low, true, 100),
        ];

        let summary = generate_summary(&results);
        assert!(summary.contains("healthy"), "got: {summary}");
        assert!(summary.contains("All 3"), "got: {summary}");
        assert!(
            summary.contains("No critical issues"),
            "should report no issues: {summary}"
        );
        // Login is the highest-importance flow
        assert!(
            summary.contains("Login"),
            "should mention top flow: {summary}"
        );
        assert!(summary.contains("fast"), "350ms should be fast: {summary}");
    }

    #[test]
    fn test_summary_some_fail() {
        let results = vec![
            make_result(
                "Login",
                FlowType::Auth,
                FlowImportance::Critical,
                false,
                350,
            ),
            make_result(
                "Search",
                FlowType::Search,
                FlowImportance::Medium,
                true,
                200,
            ),
            make_result("Nav", FlowType::Navigation, FlowImportance::Low, true, 100),
        ];

        let summary = generate_summary(&results);
        assert!(
            summary.contains("needs attention"),
            "should say needs attention: {summary}"
        );
        assert!(summary.contains("2/3"), "got: {summary}");
        assert!(
            summary.contains("Critical failure"),
            "should mention critical: {summary}"
        );
        assert!(
            summary.contains("Login"),
            "should list Login as broken: {summary}"
        );
    }

    #[test]
    fn test_summary_all_fail() {
        let results = vec![
            make_result(
                "Login",
                FlowType::Auth,
                FlowImportance::Critical,
                false,
                350,
            ),
            make_result(
                "Search",
                FlowType::Search,
                FlowImportance::Medium,
                false,
                200,
            ),
        ];

        let summary = generate_summary(&results);
        assert!(
            summary.contains("critical issues"),
            "should say critical issues: {summary}"
        );
        assert!(
            summary.contains("All 2"),
            "should say all 2 failing: {summary}"
        );
    }

    #[test]
    fn test_summary_empty() {
        let summary = generate_summary(&[]);
        assert_eq!(summary, "No flows were tested.");
    }

    #[test]
    fn test_summary_some_fail_no_critical() {
        let results = vec![
            make_result(
                "Contact Form",
                FlowType::Form,
                FlowImportance::Medium,
                false,
                1500,
            ),
            make_result("Nav", FlowType::Navigation, FlowImportance::Low, true, 100),
        ];

        let summary = generate_summary(&results);
        assert!(
            summary.contains("needs attention"),
            "should say needs attention: {summary}"
        );
        assert!(summary.contains("1/2"), "got: {summary}");
        // No critical failure sentence
        assert!(
            !summary.contains("Critical failure"),
            "no critical failures: {summary}"
        );
        assert!(
            summary.contains("Contact Form"),
            "should list broken: {summary}"
        );
    }

    // -----------------------------------------------------------------------
    // speed_callout tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_speed_callout_ms() {
        let callout = speed_callout(750);
        assert!(callout.starts_with("Scanned in 750ms"), "got: {callout}");
        assert!(callout.contains("Playwright"), "got: {callout}");
        assert!(callout.contains("Manual QA"), "got: {callout}");
    }

    #[test]
    fn test_speed_callout_seconds() {
        let callout = speed_callout(3500);
        assert!(callout.starts_with("Scanned in 3.5s"), "got: {callout}");
    }

    // -----------------------------------------------------------------------
    // html_escape tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
        assert_eq!(html_escape(r#"he said "hi""#), "he said &quot;hi&quot;");
        assert_eq!(html_escape("it's"), "it&#x27;s");
        assert_eq!(html_escape("clean"), "clean");
    }

    // -----------------------------------------------------------------------
    // Internal helper tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_url_path() {
        assert_eq!(extract_url_path("http://localhost:3000/login"), "/login");
        assert_eq!(extract_url_path("http://localhost:3000/"), "/");
        assert_eq!(extract_url_path("http://localhost:3000"), "/");
        assert_eq!(
            extract_url_path("http://localhost:3000/admin/users"),
            "/admin/users"
        );
    }

    #[test]
    fn test_page_name_from_path() {
        assert_eq!(page_name_from_path("/contact"), "Contact");
        assert_eq!(page_name_from_path("/"), "Home");
        assert_eq!(page_name_from_path("/admin/users"), "Users");
        assert_eq!(page_name_from_path("/user-settings"), "User Settings");
    }

    #[test]
    fn test_extract_name_from_selector() {
        assert_eq!(extract_name_from_selector("input[name=email]"), "email");
        assert_eq!(
            extract_name_from_selector("input[name=\"full_name\"]"),
            "full_name"
        );
        assert_eq!(extract_name_from_selector("#input-1"), "input");
    }

    #[test]
    fn test_speed_judgment() {
        assert_eq!(speed_judgment(100), "fast");
        assert_eq!(speed_judgment(499), "fast");
        assert_eq!(speed_judgment(500), "good");
        assert_eq!(speed_judgment(1999), "good");
        assert_eq!(speed_judgment(2000), "slow");
        assert_eq!(speed_judgment(4999), "slow");
        assert_eq!(speed_judgment(5000), "very slow");
        assert_eq!(speed_judgment(10000), "very slow");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(750), "750ms");
        assert_eq!(format_duration(999), "999ms");
        assert_eq!(format_duration(1000), "1.0s");
        assert_eq!(format_duration(3500), "3.5s");
    }
}
