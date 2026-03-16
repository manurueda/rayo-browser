//! Auth wall detection for transparent authentication.
//!
//! Detects when a navigation lands on a login page (either via redirect or
//! because the target page itself is a login form). Used by `goto_with_auto_auth`
//! to decide whether to attempt cookie import from the user's real browser.

use crate::page_map::PageMap;

/// Auth-related keywords found in URLs that indicate a login/SSO redirect.
const AUTH_KEYWORDS: &[&str] = &[
    "login",
    "signin",
    "sign-in",
    "sign_in",
    "auth",
    "oauth",
    "sso",
    "cas/login",
    "saml",
    "accounts.google",
    "id.atlassian",
    "identity",
    "openid",
    "authorize",
];

/// Check if a navigation resulted in an auth redirect.
///
/// Compares the requested URL with the final URL after all redirects.
/// If the final URL contains auth-related keywords and differs from the
/// requested URL, this is likely an auth redirect.
pub fn is_auth_redirect(requested_url: &str, final_url: &str) -> bool {
    if requested_url == final_url {
        return false;
    }

    let final_lower = final_url.to_lowercase();
    AUTH_KEYWORDS.iter().any(|kw| final_lower.contains(kw))
}

/// Check if a page map looks like a login form.
///
/// Primary signal: presence of a password input field.
/// A page with a password input is almost certainly a login/auth page.
pub fn is_login_page(map: &PageMap) -> bool {
    map.interactive
        .iter()
        .any(|el| el.r#type.as_deref() == Some("password"))
}

/// Extract the effective domain from a URL.
///
/// Strips `www.` prefix and leading dots. Uses simple string parsing
/// to avoid adding a dependency on the `url` crate.
///
/// # Examples
/// ```
/// # use rayo_core::auth::extract_domain;
/// assert_eq!(extract_domain("https://www.example.com/path"), Some("example.com".to_string()));
/// assert_eq!(extract_domain("https://app.github.com"), Some("app.github.com".to_string()));
/// ```
pub fn extract_domain(url: &str) -> Option<String> {
    // Find the host part: skip "scheme://" then take until "/" or ":" or "?"
    let after_scheme = url.find("://").map(|i| &url[i + 3..]).unwrap_or(url);

    let host = after_scheme
        .split('/')
        .next()?
        .split(':')
        .next()?
        .split('?')
        .next()?;

    if host.is_empty() {
        return None;
    }

    // Strip leading dots and www prefix
    let host = host.trim_start_matches('.');
    let host = host.strip_prefix("www.").unwrap_or(host);

    Some(host.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page_map::InteractiveElement;

    #[test]
    fn detect_auth_redirect_to_login() {
        assert!(is_auth_redirect(
            "https://app.example.com/dashboard",
            "https://accounts.google.com/ServiceLogin"
        ));
        assert!(is_auth_redirect(
            "https://internal.company.com/wiki",
            "https://sso.company.com/auth/login"
        ));
    }

    #[test]
    fn no_auth_redirect_when_same_url() {
        assert!(!is_auth_redirect(
            "https://example.com/login",
            "https://example.com/login"
        ));
    }

    #[test]
    fn no_auth_redirect_for_normal_navigation() {
        assert!(!is_auth_redirect(
            "https://example.com/page1",
            "https://example.com/page2"
        ));
    }

    #[test]
    fn detect_login_page_with_password_input() {
        let map = PageMap {
            url: "https://example.com/login".into(),
            title: "Login".into(),
            interactive: vec![
                InteractiveElement {
                    id: 0,
                    tag: "input".into(),
                    r#type: Some("text".into()),
                    name: Some("username".into()),
                    label: Some("Username".into()),
                    text: None,
                    placeholder: None,
                    value: None,
                    options: None,
                    role: None,
                    href: None,
                    selector: "input[name=\"username\"]".into(),
                    state: vec![],
                },
                InteractiveElement {
                    id: 1,
                    tag: "input".into(),
                    r#type: Some("password".into()),
                    name: Some("password".into()),
                    label: Some("Password".into()),
                    text: None,
                    placeholder: None,
                    value: None,
                    options: None,
                    role: None,
                    href: None,
                    selector: "input[name=\"password\"]".into(),
                    state: vec![],
                },
            ],
            headings: vec!["Sign In".into()],
            text_summary: "Please enter your credentials".into(),
            total_interactive: None,
            truncated: None,
        };

        assert!(is_login_page(&map));
    }

    #[test]
    fn non_login_page() {
        let map = PageMap {
            url: "https://example.com/dashboard".into(),
            title: "Dashboard".into(),
            interactive: vec![InteractiveElement {
                id: 0,
                tag: "input".into(),
                r#type: Some("text".into()),
                name: Some("search".into()),
                label: Some("Search".into()),
                text: None,
                placeholder: None,
                value: None,
                options: None,
                role: None,
                href: None,
                selector: "input[name=\"search\"]".into(),
                state: vec![],
            }],
            headings: vec!["Dashboard".into()],
            text_summary: "Welcome to your dashboard".into(),
            total_interactive: None,
            truncated: None,
        };

        assert!(!is_login_page(&map));
    }

    #[test]
    fn extract_domain_basic() {
        assert_eq!(
            extract_domain("https://www.example.com/path"),
            Some("example.com".to_string())
        );
        assert_eq!(
            extract_domain("https://app.github.com/settings"),
            Some("app.github.com".to_string())
        );
        assert_eq!(
            extract_domain("http://localhost:8080/test"),
            Some("localhost".to_string())
        );
    }

    #[test]
    fn extract_domain_strips_www() {
        assert_eq!(
            extract_domain("https://www.google.com"),
            Some("google.com".to_string())
        );
    }

    #[test]
    fn extract_domain_invalid_url() {
        assert_eq!(extract_domain("not a url"), Some("not a url".to_string()));
        assert_eq!(extract_domain("://"), None);
    }
}
