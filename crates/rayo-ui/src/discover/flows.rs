//! Flow detection from page maps.
//!
//! Analyzes interactive elements on a page to identify user flows
//! (forms, auth, CRUD, navigation, search) and generates step sequences.

use rayo_core::page_map::{InteractiveElement, PageMap};

/// A detected user flow that can be turned into a test.
#[derive(Debug, Clone)]
pub struct DetectedFlow {
    /// Human-readable name, e.g. "Login Form", "Search", "Create User".
    pub name: String,
    /// Classification of the flow.
    pub flow_type: FlowType,
    /// Ordered steps to execute this flow.
    pub steps: Vec<FlowStep>,
    /// The URL where this flow was detected.
    pub url: String,
}

/// Classification of a detected flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowType {
    Form,
    Auth,
    Crud,
    Navigation,
    Search,
}

impl FlowType {
    pub fn as_str(&self) -> &str {
        match self {
            FlowType::Form => "form",
            FlowType::Auth => "auth",
            FlowType::Crud => "crud",
            FlowType::Navigation => "navigation",
            FlowType::Search => "search",
        }
    }
}

/// A single step in a detected flow.
#[derive(Debug, Clone)]
pub struct FlowStep {
    pub action: String,
    pub selector: Option<String>,
    pub value: Option<String>,
    pub url: Option<String>,
    pub assertion: Option<String>,
}

/// Detect all flows from a page map and URL.
pub fn detect_flows(page_map: &PageMap, url: &str) -> Vec<DetectedFlow> {
    let mut flows = Vec::new();

    // Detect auth flow (check before generic form so we can specialize)
    if let Some(flow) = detect_auth_flow(page_map, url) {
        flows.push(flow);
    }

    // Detect search flow
    if let Some(flow) = detect_search_flow(page_map, url) {
        flows.push(flow);
    }

    // Detect generic form flows (skip if auth was already detected)
    let has_auth = flows.iter().any(|f| f.flow_type == FlowType::Auth);
    let form_flows = detect_form_flows(page_map, url, has_auth);
    flows.extend(form_flows);

    // Detect CRUD flow (form + table/list on same page)
    if let Some(flow) = detect_crud_flow(page_map, url) {
        flows.push(flow);
    }

    // Detect navigation flow
    if let Some(flow) = detect_navigation_flow(page_map, url) {
        flows.push(flow);
    }

    flows
}

/// Detect auth flow: password input or URL contains login/signin.
fn detect_auth_flow(page_map: &PageMap, url: &str) -> Option<DetectedFlow> {
    let url_lower = url.to_lowercase();
    let has_password = page_map
        .interactive
        .iter()
        .any(|el| el.r#type.as_deref() == Some("password"));
    let url_suggests_auth = url_lower.contains("login")
        || url_lower.contains("signin")
        || url_lower.contains("sign-in")
        || url_lower.contains("sign_in");

    if !has_password && !url_suggests_auth {
        return None;
    }

    let mut steps = Vec::new();

    // Navigate to the page
    steps.push(FlowStep {
        action: "navigate".into(),
        selector: None,
        value: None,
        url: Some(url.to_string()),
        assertion: None,
    });

    // Fill in form fields
    for el in &page_map.interactive {
        if el.tag != "input" {
            continue;
        }
        let input_type = el.r#type.as_deref().unwrap_or("text");
        match input_type {
            "email" | "text" => {
                let is_email = input_type == "email"
                    || el.name.as_deref().unwrap_or("").contains("email")
                    || el
                        .placeholder
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains("email");
                let value = if is_email {
                    "test@example.com"
                } else {
                    "testuser"
                };
                steps.push(FlowStep {
                    action: "type".into(),
                    selector: Some(el.selector.clone()),
                    value: Some(value.into()),
                    url: None,
                    assertion: None,
                });
            }
            "password" => {
                steps.push(FlowStep {
                    action: "type".into(),
                    selector: Some(el.selector.clone()),
                    value: Some("TestPassword123!".into()),
                    url: None,
                    assertion: None,
                });
            }
            _ => {}
        }
    }

    // Find submit button
    if let Some(submit) = find_submit_button(&page_map.interactive) {
        steps.push(FlowStep {
            action: "click".into(),
            selector: Some(submit.selector.clone()),
            value: None,
            url: None,
            assertion: None,
        });
    }

    // Assert page changed
    steps.push(FlowStep {
        action: "assert".into(),
        selector: None,
        value: None,
        url: None,
        assertion: Some("page_map_contains_any".into()),
    });

    let page_slug = slug_from_url(url);
    Some(DetectedFlow {
        name: format!("{page_slug} Login Flow"),
        flow_type: FlowType::Auth,
        steps,
        url: url.to_string(),
    })
}

/// Detect search flow: search input on the page.
fn detect_search_flow(page_map: &PageMap, url: &str) -> Option<DetectedFlow> {
    let search_input = page_map.interactive.iter().find(|el| {
        el.r#type.as_deref() == Some("search")
            || el
                .name
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .contains("search")
            || el
                .placeholder
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .contains("search")
            || el
                .label
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .contains("search")
    })?;

    let mut steps = Vec::new();

    steps.push(FlowStep {
        action: "navigate".into(),
        selector: None,
        value: None,
        url: Some(url.to_string()),
        assertion: None,
    });

    steps.push(FlowStep {
        action: "type".into(),
        selector: Some(search_input.selector.clone()),
        value: Some("test query".into()),
        url: None,
        assertion: None,
    });

    // Look for a search button, or press Enter
    let search_button = page_map.interactive.iter().find(|el| {
        let text = el.text.as_deref().unwrap_or("").to_lowercase();
        let label = el.label.as_deref().unwrap_or("").to_lowercase();
        (el.tag == "button" && (text.contains("search") || label.contains("search")))
            || el.r#type.as_deref() == Some("submit")
    });

    if let Some(btn) = search_button {
        steps.push(FlowStep {
            action: "click".into(),
            selector: Some(btn.selector.clone()),
            value: None,
            url: None,
            assertion: None,
        });
    } else {
        steps.push(FlowStep {
            action: "press".into(),
            selector: None,
            value: Some("Enter".into()),
            url: None,
            assertion: None,
        });
    }

    steps.push(FlowStep {
        action: "assert".into(),
        selector: None,
        value: None,
        url: None,
        assertion: Some("page_map_contains_any".into()),
    });

    let page_slug = slug_from_url(url);
    Some(DetectedFlow {
        name: format!("{page_slug} Search"),
        flow_type: FlowType::Search,
        steps,
        url: url.to_string(),
    })
}

/// Detect form flows: pages with inputs + submit button (excluding auth forms).
fn detect_form_flows(page_map: &PageMap, url: &str, skip_if_auth: bool) -> Vec<DetectedFlow> {
    let inputs: Vec<&InteractiveElement> = page_map
        .interactive
        .iter()
        .filter(|el| {
            el.tag == "input"
                && !matches!(
                    el.r#type.as_deref(),
                    Some("hidden") | Some("submit") | Some("button")
                )
        })
        .collect();

    let has_submit = find_submit_button(&page_map.interactive).is_some();

    if inputs.is_empty() || !has_submit {
        return Vec::new();
    }

    // Skip if this looks like an auth form we already detected
    if skip_if_auth
        && inputs
            .iter()
            .any(|el| el.r#type.as_deref() == Some("password"))
    {
        return Vec::new();
    }

    let mut steps = Vec::new();

    steps.push(FlowStep {
        action: "navigate".into(),
        selector: None,
        value: None,
        url: Some(url.to_string()),
        assertion: None,
    });

    for input in &inputs {
        let value = generate_test_value(input);
        steps.push(FlowStep {
            action: "type".into(),
            selector: Some(input.selector.clone()),
            value: Some(value),
            url: None,
            assertion: None,
        });
    }

    // Handle select elements too
    for el in &page_map.interactive {
        if el.tag == "select"
            && let Some(opts) = &el.options
            && let Some(first_opt) = opts.first()
        {
            steps.push(FlowStep {
                action: "select".into(),
                selector: Some(el.selector.clone()),
                value: Some(first_opt.clone()),
                url: None,
                assertion: None,
            });
        }
    }

    if let Some(submit) = find_submit_button(&page_map.interactive) {
        steps.push(FlowStep {
            action: "click".into(),
            selector: Some(submit.selector.clone()),
            value: None,
            url: None,
            assertion: None,
        });
    }

    steps.push(FlowStep {
        action: "assert".into(),
        selector: None,
        value: None,
        url: None,
        assertion: Some("page_map_contains_any".into()),
    });

    let page_slug = slug_from_url(url);
    vec![DetectedFlow {
        name: format!("{page_slug} Form"),
        flow_type: FlowType::Form,
        steps,
        url: url.to_string(),
    }]
}

/// Detect CRUD flow: page has both a form and a table or list.
fn detect_crud_flow(page_map: &PageMap, url: &str) -> Option<DetectedFlow> {
    let has_form_inputs = page_map
        .interactive
        .iter()
        .any(|el| el.tag == "input" && el.r#type.as_deref() != Some("hidden"));
    let has_submit = find_submit_button(&page_map.interactive).is_some();

    // Check text summary for table/list indicators
    let text = page_map.text_summary.to_lowercase();
    let has_list_hint = text.contains("table")
        || page_map.headings.iter().any(|h| {
            let lower = h.to_lowercase();
            lower.contains("list") || lower.contains("all ") || lower.contains("manage")
        });

    if !has_form_inputs || !has_submit || !has_list_hint {
        return None;
    }

    // Build a CRUD flow: fill form, submit, assert
    let mut steps = Vec::new();

    steps.push(FlowStep {
        action: "navigate".into(),
        selector: None,
        value: None,
        url: Some(url.to_string()),
        assertion: None,
    });

    for el in &page_map.interactive {
        if el.tag == "input"
            && !matches!(
                el.r#type.as_deref(),
                Some("hidden") | Some("submit") | Some("button")
            )
        {
            let value = generate_test_value(el);
            steps.push(FlowStep {
                action: "type".into(),
                selector: Some(el.selector.clone()),
                value: Some(value),
                url: None,
                assertion: None,
            });
        }
    }

    if let Some(submit) = find_submit_button(&page_map.interactive) {
        steps.push(FlowStep {
            action: "click".into(),
            selector: Some(submit.selector.clone()),
            value: None,
            url: None,
            assertion: None,
        });
    }

    steps.push(FlowStep {
        action: "assert".into(),
        selector: None,
        value: None,
        url: None,
        assertion: Some("page_map_contains_any".into()),
    });

    let page_slug = slug_from_url(url);
    Some(DetectedFlow {
        name: format!("{page_slug} CRUD"),
        flow_type: FlowType::Crud,
        steps,
        url: url.to_string(),
    })
}

/// Detect navigation flow: page has multiple nav links.
fn detect_navigation_flow(page_map: &PageMap, url: &str) -> Option<DetectedFlow> {
    let nav_links: Vec<&InteractiveElement> = page_map
        .interactive
        .iter()
        .filter(|el| {
            el.tag == "a"
                && el.href.is_some()
                && el.text.is_some()
                && !el.href.as_deref().unwrap_or("").starts_with("http")
                || el.href.as_deref().unwrap_or("").starts_with(
                    url.split('/')
                        .take(3)
                        .collect::<Vec<_>>()
                        .join("/")
                        .as_str(),
                )
        })
        .take(10) // Cap at 10 links for navigation test
        .collect();

    if nav_links.len() < 2 {
        return None;
    }

    let mut steps = Vec::new();

    steps.push(FlowStep {
        action: "navigate".into(),
        selector: None,
        value: None,
        url: Some(url.to_string()),
        assertion: None,
    });

    for link in &nav_links {
        steps.push(FlowStep {
            action: "click".into(),
            selector: Some(link.selector.clone()),
            value: None,
            url: None,
            assertion: None,
        });
        steps.push(FlowStep {
            action: "assert".into(),
            selector: None,
            value: None,
            url: None,
            assertion: Some("page_map_contains_any".into()),
        });
    }

    let page_slug = slug_from_url(url);
    Some(DetectedFlow {
        name: format!("{page_slug} Navigation"),
        flow_type: FlowType::Navigation,
        steps,
        url: url.to_string(),
    })
}

/// Find a submit button among interactive elements.
fn find_submit_button(elements: &[InteractiveElement]) -> Option<&InteractiveElement> {
    // First look for an explicit submit button
    elements
        .iter()
        .find(|el| {
            el.r#type.as_deref() == Some("submit")
                || (el.tag == "button"
                    && el
                        .text
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains("submit"))
        })
        .or_else(|| {
            // Fall back to any button
            elements.iter().find(|el| {
                el.tag == "button"
                    && el.r#type.as_deref() != Some("button")
                    && el.r#type.as_deref() != Some("reset")
            })
        })
}

/// Generate a realistic test value based on the input field's metadata.
fn generate_test_value(el: &InteractiveElement) -> String {
    let input_type = el.r#type.as_deref().unwrap_or("text");
    let name = el.name.as_deref().unwrap_or("").to_lowercase();
    let placeholder = el.placeholder.as_deref().unwrap_or("").to_lowercase();
    let label = el.label.as_deref().unwrap_or("").to_lowercase();

    let hint = format!("{name} {placeholder} {label}");

    match input_type {
        "email" => "test@example.com".into(),
        "password" => "TestPassword123!".into(),
        "tel" | "phone" => "555-0123".into(),
        "number" => "42".into(),
        "url" => "https://example.com".into(),
        "date" => "2025-01-15".into(),
        _ => {
            // Infer from name/placeholder/label
            if hint.contains("email") {
                "test@example.com".into()
            } else if hint.contains("phone") || hint.contains("tel") {
                "555-0123".into()
            } else if hint.contains("name") {
                "Test User".into()
            } else if hint.contains("address") {
                "123 Test Street".into()
            } else if hint.contains("city") {
                "Test City".into()
            } else if hint.contains("zip") || hint.contains("postal") {
                "12345".into()
            } else if hint.contains("company") || hint.contains("org") {
                "Test Company".into()
            } else {
                "Test input".into()
            }
        }
    }
}

/// Extract a URL-friendly slug from a URL path.
pub fn slug_from_url(url: &str) -> String {
    // Parse path from URL
    let path = if let Some(idx) = url.find("://") {
        let after_scheme = &url[idx + 3..];
        if let Some(slash_idx) = after_scheme.find('/') {
            &after_scheme[slash_idx..]
        } else {
            "/"
        }
    } else {
        url
    };

    let slug = path
        .trim_matches('/')
        .replace('/', "-")
        .replace([':', '?', '&', '=', '#'], "");

    if slug.is_empty() { "home".into() } else { slug }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slug_from_url() {
        assert_eq!(slug_from_url("http://localhost:3000/"), "home");
        assert_eq!(slug_from_url("http://localhost:3000/login"), "login");
        assert_eq!(
            slug_from_url("http://localhost:3000/dashboard/settings"),
            "dashboard-settings"
        );
    }

    #[test]
    fn test_generate_test_value() {
        let email_input = InteractiveElement {
            id: 0,
            tag: "input".into(),
            r#type: Some("email".into()),
            name: Some("email".into()),
            label: None,
            text: None,
            placeholder: None,
            value: None,
            options: None,
            role: None,
            href: None,
            selector: "input[name=email]".into(),
            state: vec![],
            bbox: None,
        };
        assert_eq!(generate_test_value(&email_input), "test@example.com");

        let name_input = InteractiveElement {
            id: 1,
            tag: "input".into(),
            r#type: Some("text".into()),
            name: Some("full_name".into()),
            label: None,
            text: None,
            placeholder: None,
            value: None,
            options: None,
            role: None,
            href: None,
            selector: "input[name=full_name]".into(),
            state: vec![],
            bbox: None,
        };
        assert_eq!(generate_test_value(&name_input), "Test User");
    }

    /// Helper to create an InteractiveElement with defaults.
    fn make_element(
        id: usize,
        tag: &str,
        r#type: Option<&str>,
        name: Option<&str>,
        text: Option<&str>,
        href: Option<&str>,
        placeholder: Option<&str>,
        label: Option<&str>,
    ) -> InteractiveElement {
        InteractiveElement {
            id,
            tag: tag.into(),
            r#type: r#type.map(|s| s.into()),
            name: name.map(|s| s.into()),
            label: label.map(|s| s.into()),
            text: text.map(|s| s.into()),
            placeholder: placeholder.map(|s| s.into()),
            value: None,
            options: None,
            role: None,
            href: href.map(|s| s.into()),
            selector: format!("#{tag}-{id}"),
            state: vec![],
            bbox: None,
        }
    }

    #[test]
    fn test_detect_auth_flow() {
        let page_map = PageMap {
            url: "http://localhost:3000/login".into(),
            title: "Login".into(),
            interactive: vec![
                make_element(
                    0,
                    "input",
                    Some("email"),
                    Some("email"),
                    None,
                    None,
                    Some("Email"),
                    None,
                ),
                make_element(
                    1,
                    "input",
                    Some("password"),
                    Some("password"),
                    None,
                    None,
                    Some("Password"),
                    None,
                ),
                make_element(
                    2,
                    "button",
                    Some("submit"),
                    None,
                    Some("Sign In"),
                    None,
                    None,
                    None,
                ),
            ],
            headings: vec!["Login".into()],
            text_summary: "Login page".into(),
            total_interactive: None,
            truncated: None,
        };

        let flows = detect_flows(&page_map, "http://localhost:3000/login");

        let auth_flows: Vec<&DetectedFlow> = flows
            .iter()
            .filter(|f| f.flow_type == FlowType::Auth)
            .collect();
        assert_eq!(auth_flows.len(), 1, "Should detect exactly one Auth flow");

        let auth = &auth_flows[0];
        assert_eq!(auth.flow_type, FlowType::Auth);
        assert!(auth.name.contains("Login Flow"));

        // Should have navigate, type email, type password, click submit, assert steps
        let actions: Vec<&str> = auth.steps.iter().map(|s| s.action.as_str()).collect();
        assert!(actions.contains(&"navigate"));
        assert!(actions.contains(&"type"));
        assert!(actions.contains(&"click"));
        assert!(actions.contains(&"assert"));
    }

    #[test]
    fn test_detect_form_flow_without_password() {
        let page_map = PageMap {
            url: "http://localhost:3000/contact".into(),
            title: "Contact Us".into(),
            interactive: vec![
                make_element(
                    0,
                    "input",
                    Some("text"),
                    Some("name"),
                    None,
                    None,
                    Some("Your name"),
                    None,
                ),
                make_element(
                    1,
                    "input",
                    Some("email"),
                    Some("email"),
                    None,
                    None,
                    Some("Email"),
                    None,
                ),
                make_element(
                    2,
                    "button",
                    Some("submit"),
                    None,
                    Some("Submit"),
                    None,
                    None,
                    None,
                ),
            ],
            headings: vec!["Contact Us".into()],
            text_summary: "Contact form".into(),
            total_interactive: None,
            truncated: None,
        };

        let flows = detect_flows(&page_map, "http://localhost:3000/contact");

        let form_flows: Vec<&DetectedFlow> = flows
            .iter()
            .filter(|f| f.flow_type == FlowType::Form)
            .collect();
        assert_eq!(form_flows.len(), 1, "Should detect exactly one Form flow");

        // Should NOT detect Auth flow (no password field, URL not login-related)
        let auth_flows: Vec<&DetectedFlow> = flows
            .iter()
            .filter(|f| f.flow_type == FlowType::Auth)
            .collect();
        assert_eq!(
            auth_flows.len(),
            0,
            "Should not detect Auth flow without password"
        );
    }

    #[test]
    fn test_detect_search_flow() {
        let page_map = PageMap {
            url: "http://localhost:3000/".into(),
            title: "Home".into(),
            interactive: vec![make_element(
                0,
                "input",
                Some("search"),
                Some("q"),
                None,
                None,
                Some("Search..."),
                None,
            )],
            headings: vec!["Welcome".into()],
            text_summary: "Home page with search".into(),
            total_interactive: None,
            truncated: None,
        };

        let flows = detect_flows(&page_map, "http://localhost:3000/");

        let search_flows: Vec<&DetectedFlow> = flows
            .iter()
            .filter(|f| f.flow_type == FlowType::Search)
            .collect();
        assert_eq!(
            search_flows.len(),
            1,
            "Should detect exactly one Search flow"
        );

        let search = &search_flows[0];
        // Should have navigate, type search query, press Enter (no submit button), assert
        let actions: Vec<&str> = search.steps.iter().map(|s| s.action.as_str()).collect();
        assert!(actions.contains(&"navigate"));
        assert!(actions.contains(&"type"));
        assert!(
            actions.contains(&"press"),
            "Without submit button, should press Enter"
        );
        assert!(actions.contains(&"assert"));
    }

    #[test]
    fn test_detect_search_flow_by_placeholder() {
        let page_map = PageMap {
            url: "http://localhost:3000/".into(),
            title: "Home".into(),
            interactive: vec![make_element(
                0,
                "input",
                Some("text"),
                Some("query"),
                None,
                None,
                Some("Search products..."),
                None,
            )],
            headings: vec![],
            text_summary: "".into(),
            total_interactive: None,
            truncated: None,
        };

        let flows = detect_flows(&page_map, "http://localhost:3000/");

        let search_flows: Vec<&DetectedFlow> = flows
            .iter()
            .filter(|f| f.flow_type == FlowType::Search)
            .collect();
        assert_eq!(
            search_flows.len(),
            1,
            "Should detect search by placeholder text"
        );
    }

    #[test]
    fn test_detect_navigation_flow() {
        let page_map = PageMap {
            url: "http://localhost:3000/".into(),
            title: "Home".into(),
            interactive: vec![
                make_element(
                    0,
                    "a",
                    None,
                    None,
                    Some("About"),
                    Some("/about"),
                    None,
                    None,
                ),
                make_element(
                    1,
                    "a",
                    None,
                    None,
                    Some("Contact"),
                    Some("/contact"),
                    None,
                    None,
                ),
                make_element(2, "a", None, None, Some("Blog"), Some("/blog"), None, None),
            ],
            headings: vec!["Home".into()],
            text_summary: "Home page".into(),
            total_interactive: None,
            truncated: None,
        };

        let flows = detect_flows(&page_map, "http://localhost:3000/");

        let nav_flows: Vec<&DetectedFlow> = flows
            .iter()
            .filter(|f| f.flow_type == FlowType::Navigation)
            .collect();
        assert_eq!(
            nav_flows.len(),
            1,
            "Should detect exactly one Navigation flow"
        );

        let nav = &nav_flows[0];
        // Navigation flow should have navigate + click/assert pairs for each link
        let click_count = nav.steps.iter().filter(|s| s.action == "click").count();
        assert!(
            click_count >= 2,
            "Navigation flow should click at least 2 links"
        );
    }

    #[test]
    fn test_empty_page_no_flows() {
        let page_map = PageMap {
            url: "http://localhost:3000/empty".into(),
            title: "Empty".into(),
            interactive: vec![],
            headings: vec![],
            text_summary: "".into(),
            total_interactive: None,
            truncated: None,
        };

        let flows = detect_flows(&page_map, "http://localhost:3000/empty");
        assert!(flows.is_empty(), "Empty page map should produce no flows");
    }

    #[test]
    fn test_find_submit_button_explicit() {
        let elements = vec![
            make_element(
                0,
                "input",
                Some("text"),
                Some("name"),
                None,
                None,
                None,
                None,
            ),
            make_element(
                1,
                "button",
                Some("submit"),
                None,
                Some("Go"),
                None,
                None,
                None,
            ),
        ];

        let submit = find_submit_button(&elements);
        assert!(submit.is_some());
        assert_eq!(submit.unwrap().id, 1);
    }

    #[test]
    fn test_find_submit_button_fallback() {
        let elements = vec![
            make_element(
                0,
                "input",
                Some("text"),
                Some("name"),
                None,
                None,
                None,
                None,
            ),
            make_element(1, "button", None, None, Some("Save"), None, None, None),
        ];

        let submit = find_submit_button(&elements);
        assert!(submit.is_some(), "Should fall back to any button");
        assert_eq!(submit.unwrap().id, 1);
    }

    #[test]
    fn test_generate_test_value_various_types() {
        let tel = make_element(0, "input", Some("tel"), None, None, None, None, None);
        assert_eq!(generate_test_value(&tel), "555-0123");

        let number = make_element(1, "input", Some("number"), None, None, None, None, None);
        assert_eq!(generate_test_value(&number), "42");

        let url = make_element(2, "input", Some("url"), None, None, None, None, None);
        assert_eq!(generate_test_value(&url), "https://example.com");

        let date = make_element(3, "input", Some("date"), None, None, None, None, None);
        assert_eq!(generate_test_value(&date), "2025-01-15");

        let password = make_element(4, "input", Some("password"), None, None, None, None, None);
        assert_eq!(generate_test_value(&password), "TestPassword123!");
    }
}
