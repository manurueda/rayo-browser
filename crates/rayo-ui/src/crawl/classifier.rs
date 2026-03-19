//! Page classification from page map analysis.
//!
//! Uses weighted signals (URL keywords, interactive elements, headings, text)
//! to classify pages as Auth, Paywall, Dashboard, etc.

use super::graph::PageType;
use rayo_core::page_map::PageMap;

/// Classify a page based on its page map and URL.
pub fn classify_page(page_map: &PageMap, url: &str) -> PageType {
    let scores = compute_scores(page_map, url);

    // Return the type with the highest score (minimum threshold 2)
    let mut best = (PageType::Content, 0i32);
    for (page_type, score) in scores {
        if score > best.1 {
            best = (page_type, score);
        }
    }

    if best.1 >= 2 {
        best.0
    } else {
        PageType::Content
    }
}

/// Compute classification scores for each page type.
fn compute_scores(page_map: &PageMap, url: &str) -> Vec<(PageType, i32)> {
    let url_lower = url.to_lowercase();
    let title_lower = page_map.title.to_lowercase();
    let text_lower = page_map.text_summary.to_lowercase();
    let headings_lower: Vec<String> = page_map.headings.iter().map(|h| h.to_lowercase()).collect();
    let all_text = format!("{title_lower} {text_lower} {}", headings_lower.join(" "));

    let mut scores: Vec<(PageType, i32)> = vec![
        (PageType::Landing, 0),
        (PageType::Auth, 0),
        (PageType::Paywall, 0),
        (PageType::Dashboard, 0),
        (PageType::Settings, 0),
        (PageType::Content, 0),
        (PageType::Error, 0),
    ];

    let mut add = |page_type: PageType, points: i32| {
        if let Some(entry) = scores.iter_mut().find(|(t, _)| *t == page_type) {
            entry.1 += points;
        }
    };

    // --- URL signals (strong) ---
    if url_lower.contains("/login")
        || url_lower.contains("/signin")
        || url_lower.contains("/sign-in")
        || url_lower.contains("/sign_in")
        || url_lower.contains("/register")
        || url_lower.contains("/signup")
    {
        add(PageType::Auth, 5);
    }

    if url_lower.contains("/pricing")
        || url_lower.contains("/plans")
        || url_lower.contains("/upgrade")
        || url_lower.contains("/subscribe")
        || url_lower.contains("/billing")
    {
        add(PageType::Paywall, 5);
    }

    if url_lower.contains("/dashboard") || url_lower.contains("/app/") {
        add(PageType::Dashboard, 4);
    }

    if url_lower.contains("/settings")
        || url_lower.contains("/profile")
        || url_lower.contains("/account")
        || url_lower.contains("/preferences")
    {
        add(PageType::Settings, 5);
    }

    if url_lower.contains("/404") || url_lower.contains("/500") {
        add(PageType::Error, 5);
    }

    // Root URL or "/" is likely landing
    let path = url_lower
        .find("://")
        .and_then(|i| url_lower[i + 3..].find('/'))
        .map(|i| &url_lower[url_lower.find("://").unwrap() + 3 + i..])
        .unwrap_or("/");
    if path == "/" || path.is_empty() {
        add(PageType::Landing, 3);
    }

    // --- Element signals ---
    let has_password = page_map
        .interactive
        .iter()
        .any(|el| el.r#type.as_deref() == Some("password"));
    if has_password {
        add(PageType::Auth, 4);
    }

    // Credit card / payment fields
    let has_payment_fields = page_map.interactive.iter().any(|el| {
        let name = el.name.as_deref().unwrap_or("").to_lowercase();
        let placeholder = el.placeholder.as_deref().unwrap_or("").to_lowercase();
        let label = el.label.as_deref().unwrap_or("").to_lowercase();
        let hint = format!("{name} {placeholder} {label}");
        hint.contains("card")
            || hint.contains("cvv")
            || hint.contains("cvc")
            || hint.contains("expir")
            || hint.contains("credit")
    });
    if has_payment_fields {
        add(PageType::Paywall, 3);
    }

    // Many navigation links + few forms → dashboard
    let link_count = page_map
        .interactive
        .iter()
        .filter(|el| el.tag == "a")
        .count();
    let input_count = page_map
        .interactive
        .iter()
        .filter(|el| el.tag == "input")
        .count();
    if link_count > 5 && input_count <= 2 {
        add(PageType::Dashboard, 2);
    }

    // --- Text/heading signals ---
    if all_text.contains("sign in")
        || all_text.contains("log in")
        || all_text.contains("forgot password")
        || all_text.contains("create account")
    {
        add(PageType::Auth, 3);
    }

    if all_text.contains("pricing")
        || all_text.contains("per month")
        || all_text.contains("/mo")
        || all_text.contains("free trial")
        || all_text.contains("upgrade")
        || all_text.contains("subscribe")
        || all_text.contains("premium")
    {
        add(PageType::Paywall, 3);
    }

    if all_text.contains("not found")
        || all_text.contains("404")
        || all_text.contains("page not found")
        || all_text.contains("server error")
        || all_text.contains("500")
    {
        add(PageType::Error, 4);
    }

    if all_text.contains("settings")
        || all_text.contains("preferences")
        || all_text.contains("change password")
        || all_text.contains("notification")
    {
        add(PageType::Settings, 2);
    }

    scores
}

#[cfg(test)]
mod tests {
    use super::*;
    use rayo_core::page_map::{InteractiveElement, PageMap};

    fn make_page_map(
        url: &str,
        title: &str,
        headings: Vec<&str>,
        text: &str,
        elements: Vec<InteractiveElement>,
    ) -> PageMap {
        PageMap {
            url: url.to_string(),
            title: title.to_string(),
            interactive: elements,
            headings: headings.into_iter().map(String::from).collect(),
            text_summary: text.to_string(),
            total_interactive: None,
            truncated: None,
        }
    }

    fn make_input(name: &str, input_type: &str) -> InteractiveElement {
        InteractiveElement {
            id: 0,
            tag: "input".to_string(),
            r#type: Some(input_type.to_string()),
            name: Some(name.to_string()),
            label: None,
            text: None,
            placeholder: None,
            value: None,
            options: None,
            role: None,
            href: None,
            selector: format!("input[name={name}]"),
            state: vec![],
            bbox: None,
        }
    }

    #[test]
    fn test_classify_login_page() {
        let pm = make_page_map(
            "http://localhost:3000/login",
            "Login",
            vec!["Sign In"],
            "Please log in to continue",
            vec![
                make_input("email", "email"),
                make_input("password", "password"),
            ],
        );
        assert_eq!(
            classify_page(&pm, "http://localhost:3000/login"),
            PageType::Auth
        );
    }

    #[test]
    fn test_classify_pricing_page() {
        let pm = make_page_map(
            "http://localhost:3000/pricing",
            "Pricing",
            vec!["Choose Your Plan"],
            "Starting at $9/mo. Free trial available.",
            vec![],
        );
        assert_eq!(
            classify_page(&pm, "http://localhost:3000/pricing"),
            PageType::Paywall
        );
    }

    #[test]
    fn test_classify_dashboard() {
        let mut links = Vec::new();
        for i in 0..8 {
            links.push(InteractiveElement {
                id: i,
                tag: "a".to_string(),
                r#type: None,
                name: None,
                label: None,
                text: Some(format!("Link {i}")),
                placeholder: None,
                value: None,
                options: None,
                role: None,
                href: Some(format!("/page-{i}")),
                selector: format!("a:nth-of-type({i})"),
                state: vec![],
                bbox: None,
            });
        }
        let pm = make_page_map(
            "http://localhost:3000/dashboard",
            "Dashboard",
            vec!["Dashboard"],
            "Welcome back",
            links,
        );
        assert_eq!(
            classify_page(&pm, "http://localhost:3000/dashboard"),
            PageType::Dashboard
        );
    }

    #[test]
    fn test_classify_error_page() {
        let pm = make_page_map(
            "http://localhost:3000/not-here",
            "Page Not Found",
            vec!["404"],
            "The page you are looking for was not found.",
            vec![],
        );
        assert_eq!(
            classify_page(&pm, "http://localhost:3000/not-here"),
            PageType::Error
        );
    }

    #[test]
    fn test_classify_settings() {
        let pm = make_page_map(
            "http://localhost:3000/settings",
            "Settings",
            vec!["Account Settings"],
            "Change password, notification preferences",
            vec![make_input("name", "text")],
        );
        assert_eq!(
            classify_page(&pm, "http://localhost:3000/settings"),
            PageType::Settings
        );
    }

    #[test]
    fn test_classify_landing() {
        let pm = make_page_map(
            "http://localhost:3000/",
            "Welcome to MyApp",
            vec!["Build faster"],
            "The best tool for developers",
            vec![],
        );
        assert_eq!(
            classify_page(&pm, "http://localhost:3000/"),
            PageType::Landing
        );
    }

    #[test]
    fn test_classify_generic_content() {
        let pm = make_page_map(
            "http://localhost:3000/about",
            "About Us",
            vec!["Our Story"],
            "We are a team of developers building great software.",
            vec![],
        );
        assert_eq!(
            classify_page(&pm, "http://localhost:3000/about"),
            PageType::Content
        );
    }
}
