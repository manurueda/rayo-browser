//! Auth wall detection for transparent authentication.
//!
//! Detects when a navigation lands on a login page (either via redirect or
//! because the target page itself is a login form). Used by `goto_with_auto_auth`
//! to decide whether to attempt cookie import from the user's real browser.

use std::future::Future;
use std::pin::Pin;

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

/// OAuth provider domains found in href attributes on login pages.
const OAUTH_PROVIDER_DOMAINS: &[&str] = &[
    "accounts.google.com",
    "github.com/login/oauth",
    "login.microsoftonline.com",
    "appleid.apple.com",
    "facebook.com/dialog/oauth",
    "login.salesforce.com",
    "auth0.com",
    "okta.com",
    "onelogin.com",
    "login.yahoo.com",
    "api.twitter.com/oauth",
    "discord.com/oauth2",
    "slack.com/oauth",
    "gitlab.com/oauth",
    "bitbucket.org/site/oauth2",
];

/// Text patterns in button/link text indicating OAuth login.
const OAUTH_TEXT_PATTERNS: &[&str] = &[
    "sign in with",
    "log in with",
    "login with",
    "continue with",
    "connect with",
    "sign in using",
    "sso",
    "single sign-on",
    "sign in to continue",
    "log in to continue",
];

/// Heading text patterns indicating a login page.
const AUTH_HEADING_PATTERNS: &[&str] = &[
    "sign in",
    "log in",
    "login",
    "welcome back",
    "authenticate",
    "enter your credentials",
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

/// Result of auth wall detection with confidence scoring.
#[derive(Debug, Clone)]
pub struct AuthDetection {
    /// Whether an auth wall was detected (confidence >= threshold).
    pub detected: bool,
    /// Individual signals that contributed.
    pub signals: Vec<AuthSignal>,
    /// Total confidence score (0.0 - 1.0).
    pub confidence: f64,
}

/// Individual signal that contributed to auth detection.
#[derive(Debug, Clone)]
pub enum AuthSignal {
    /// Final URL contains auth keywords.
    AuthKeywordInUrl,
    /// Page has a password input field.
    PasswordInput,
    /// URL path changed meaningfully.
    SuspiciousPathChange {
        requested: String,
        final_path: String,
    },
    /// Domain changed during redirect.
    CrossDomainRedirect { from: String, to: String },
    /// Page contains links to OAuth provider domains.
    OAuthProviderLinks(Vec<String>),
    /// Page contains buttons/links with OAuth text patterns.
    OAuthTextInElements(Vec<String>),
    /// Page headings contain auth-related text.
    AuthHeadings(Vec<String>),
    /// Page title contains auth-related text.
    AuthTitle(String),
}

/// Callback type for LLM-powered auth detection via MCP sampling.
///
/// Takes a base64-encoded JPEG screenshot, returns `Some(true)` if the page
/// is a login page, `Some(false)` if not, `None` on error/timeout.
/// Defined here so rayo-core stays free of rmcp dependency.
pub type LlmAuthChecker =
    Box<dyn Fn(String) -> Pin<Box<dyn Future<Output = Option<bool>> + Send>> + Send + Sync>;

const CONFIDENCE_THRESHOLD: f64 = 0.5;

/// Detect auth walls using weighted confidence scoring across multiple signals.
///
/// Combines URL analysis, page content inspection, and structural signals
/// to determine if a page is an authentication/login page.
pub fn detect_auth_wall(requested_url: &str, final_url: &str, map: &PageMap) -> AuthDetection {
    let mut signals = Vec::new();
    let mut confidence: f64 = 0.0;

    // --- URL-based signals ---
    if requested_url != final_url {
        // Signal 1: Auth keywords in final URL (weight: 0.8)
        let final_lower = final_url.to_lowercase();
        if AUTH_KEYWORDS.iter().any(|kw| final_lower.contains(kw)) {
            confidence += 0.8;
            signals.push(AuthSignal::AuthKeywordInUrl);
        }

        // Signal 2: Suspicious path change (weight: 0.3)
        if let Some(sig) = check_path_change(requested_url, final_url) {
            confidence += 0.3;
            signals.push(sig);
        }

        // Signal 3: Cross-domain redirect (weight: 0.2)
        if let (Some(req_domain), Some(fin_domain)) =
            (extract_domain(requested_url), extract_domain(final_url))
            && req_domain != fin_domain
        {
            confidence += 0.2;
            signals.push(AuthSignal::CrossDomainRedirect {
                from: req_domain,
                to: fin_domain,
            });
        }
    }

    // --- Page content signals ---

    // Signal 4: Password input (weight: 0.9)
    if map
        .interactive
        .iter()
        .any(|el| el.r#type.as_deref() == Some("password"))
    {
        confidence += 0.9;
        signals.push(AuthSignal::PasswordInput);
    }

    // Signal 5: OAuth provider links in hrefs (weight: 0.7)
    let oauth_links = find_oauth_links(map);
    if !oauth_links.is_empty() {
        confidence += 0.7;
        signals.push(AuthSignal::OAuthProviderLinks(oauth_links));
    }

    // Signal 6: OAuth text in buttons/links (weight: 0.6)
    let oauth_text = find_oauth_text(map);
    if !oauth_text.is_empty() {
        confidence += 0.6;
        signals.push(AuthSignal::OAuthTextInElements(oauth_text));
    }

    // Signal 7: Auth-related headings (weight: 0.3)
    let auth_headings: Vec<String> = map
        .headings
        .iter()
        .filter(|h| {
            let lower = h.to_lowercase();
            AUTH_HEADING_PATTERNS.iter().any(|p| lower.contains(p))
        })
        .cloned()
        .collect();
    if !auth_headings.is_empty() {
        confidence += 0.3;
        signals.push(AuthSignal::AuthHeadings(auth_headings));
    }

    // Signal 8: Auth-related title (weight: 0.2)
    let title_lower = map.title.to_lowercase();
    if AUTH_HEADING_PATTERNS
        .iter()
        .any(|p| title_lower.contains(p))
    {
        confidence += 0.2;
        signals.push(AuthSignal::AuthTitle(map.title.clone()));
    }

    AuthDetection {
        detected: confidence.min(1.0) >= CONFIDENCE_THRESHOLD,
        signals,
        confidence: confidence.min(1.0),
    }
}

fn find_oauth_links(map: &PageMap) -> Vec<String> {
    let mut found = Vec::new();
    for el in &map.interactive {
        if let Some(href) = &el.href {
            let href_lower = href.to_lowercase();
            for provider in OAUTH_PROVIDER_DOMAINS {
                if href_lower.contains(provider) {
                    found.push(provider.to_string());
                    break;
                }
            }
        }
    }
    found.dedup();
    found
}

fn find_oauth_text(map: &PageMap) -> Vec<String> {
    let mut found = Vec::new();
    for el in &map.interactive {
        for text in [el.text.as_deref(), el.label.as_deref()]
            .into_iter()
            .flatten()
        {
            let lower = text.to_lowercase();
            if OAUTH_TEXT_PATTERNS.iter().any(|p| lower.contains(p)) {
                found.push(text.to_string());
                break;
            }
        }
    }
    found.dedup();
    found
}

/// Normalize a URL for redirect comparison.
/// Strips trailing slashes, www., scheme, and fragments.
fn normalize_url(url: &str) -> String {
    let url = url.split('#').next().unwrap_or(url);

    let rest = if let Some(idx) = url.find("://") {
        &url[idx + 3..]
    } else {
        url
    };

    let (host_port, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };

    let host = host_port
        .split(':')
        .next()
        .unwrap_or(host_port)
        .to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);

    let path = if path.len() > 1 {
        path.trim_end_matches('/')
    } else {
        path
    };

    format!("{host}{path}")
}

/// Check if a redirect is meaningful (not trivial URL normalization).
pub fn is_meaningful_redirect(requested_url: &str, final_url: &str) -> bool {
    if requested_url == final_url {
        return false;
    }
    normalize_url(requested_url) != normalize_url(final_url)
}

fn check_path_change(requested_url: &str, final_url: &str) -> Option<AuthSignal> {
    let req_norm = normalize_url(requested_url);
    let fin_norm = normalize_url(final_url);

    if req_norm == fin_norm {
        return None;
    }

    // Extract paths (everything after the host)
    let req_path = req_norm.find('/').map(|i| &req_norm[i..]).unwrap_or("/");
    let fin_path = fin_norm.find('/').map(|i| &fin_norm[i..]).unwrap_or("/");

    if req_path == fin_path {
        return None;
    }

    // Same host, shallow redirect (both depth <= 1) — not suspicious enough alone
    let req_host = req_norm.split('/').next().unwrap_or("");
    let fin_host = fin_norm.split('/').next().unwrap_or("");
    let req_depth = req_path.matches('/').count();
    let fin_depth = fin_path.matches('/').count();

    if req_host == fin_host && req_depth <= 1 && fin_depth <= 1 {
        return None;
    }

    Some(AuthSignal::SuspiciousPathChange {
        requested: req_path.to_string(),
        final_path: fin_path.to_string(),
    })
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
                    bbox: None,
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
                    bbox: None,
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
                bbox: None,
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

    // --- detect_auth_wall tests ---

    fn empty_map(url: &str) -> PageMap {
        PageMap {
            url: url.into(),
            title: String::new(),
            interactive: vec![],
            headings: vec![],
            text_summary: String::new(),
            total_interactive: None,
            truncated: None,
        }
    }

    #[test]
    fn detect_auth_keyword_redirect() {
        let map = empty_map("https://accounts.google.com/ServiceLogin");
        let result = detect_auth_wall(
            "https://app.example.com/dashboard",
            "https://accounts.google.com/ServiceLogin",
            &map,
        );
        assert!(result.detected);
        assert!(result.confidence >= 0.5);
    }

    #[test]
    fn detect_password_input_page() {
        let map = PageMap {
            url: "https://example.com/login".into(),
            title: "Login".into(),
            interactive: vec![InteractiveElement {
                id: 0,
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
                selector: "input[type=password]".into(),
                state: vec![],
                bbox: None,
            }],
            headings: vec![],
            text_summary: String::new(),
            total_interactive: None,
            truncated: None,
        };
        let result = detect_auth_wall(
            "https://example.com/login",
            "https://example.com/login",
            &map,
        );
        assert!(result.detected);
    }

    #[test]
    fn detect_oauth_provider_link() {
        let map = PageMap {
            url: "https://app.example.com/login".into(),
            title: "Welcome".into(),
            interactive: vec![InteractiveElement {
                id: 0,
                tag: "a".into(),
                r#type: None,
                name: None,
                label: None,
                text: Some("Sign in with Google".into()),
                placeholder: None,
                value: None,
                options: None,
                role: None,
                href: Some("https://accounts.google.com/o/oauth2/auth?client_id=abc".into()),
                selector: "a".into(),
                state: vec![],
                bbox: None,
            }],
            headings: vec![],
            text_summary: String::new(),
            total_interactive: None,
            truncated: None,
        };
        let result = detect_auth_wall(
            "https://app.example.com/login",
            "https://app.example.com/login",
            &map,
        );
        // OAuth link (0.7) + OAuth text (0.6) = 1.0 (capped)
        assert!(result.detected);
    }

    #[test]
    fn detect_path_change_plus_heading() {
        let mut map = empty_map("https://app.example.com/");
        map.headings = vec!["Sign In to Continue".into()];
        let result = detect_auth_wall(
            "https://app.example.com/dashboard/settings",
            "https://app.example.com/",
            &map,
        );
        // path change (0.3) + heading (0.3) = 0.6 >= 0.5
        assert!(result.detected);
    }

    #[test]
    fn path_change_alone_not_detected() {
        let map = empty_map("https://app.example.com/");
        let result = detect_auth_wall(
            "https://app.example.com/dashboard/settings",
            "https://app.example.com/",
            &map,
        );
        // path change alone = 0.3 < 0.5
        assert!(!result.detected);
    }

    #[test]
    fn normal_page_zero_confidence() {
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
                selector: "input".into(),
                state: vec![],
                bbox: None,
            }],
            headings: vec!["Dashboard".into()],
            text_summary: String::new(),
            total_interactive: None,
            truncated: None,
        };
        let result = detect_auth_wall(
            "https://example.com/dashboard",
            "https://example.com/dashboard",
            &map,
        );
        assert!(!result.detected);
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn no_false_positive_trailing_slash() {
        assert!(!is_meaningful_redirect(
            "https://example.com/path",
            "https://example.com/path/"
        ));
    }

    #[test]
    fn no_false_positive_www() {
        assert!(!is_meaningful_redirect(
            "https://www.example.com/path",
            "https://example.com/path"
        ));
    }

    #[test]
    fn no_false_positive_https_upgrade() {
        assert!(!is_meaningful_redirect(
            "http://example.com/path",
            "https://example.com/path"
        ));
    }

    #[test]
    fn meaningful_redirect_different_path() {
        assert!(is_meaningful_redirect(
            "https://www.company.inc/play",
            "https://www.company.inc/"
        ));
    }

    #[test]
    fn no_redirect_same_url() {
        assert!(!is_meaningful_redirect(
            "https://example.com/page",
            "https://example.com/page"
        ));
    }

    #[test]
    fn no_false_positive_fragment() {
        assert!(!is_meaningful_redirect(
            "https://example.com/page",
            "https://example.com/page#section"
        ));
    }
}
