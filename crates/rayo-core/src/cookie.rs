//! Cookie types owned by rayo-core.
//!
//! These decouple downstream crates from chromiumoxide's internal CDP types.

use serde::{Deserialize, Serialize};

/// Input for setting a cookie. Converted to CDP CookieParam internally.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCookie {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secure: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub same_site: Option<SameSite>,
    /// Expiration as seconds since Unix epoch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<f64>,
}

/// SameSite cookie attribute.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SameSite {
    Strict,
    Lax,
    None,
}

/// Cookie returned from the browser. Read-only view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieInfo {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub same_site: Option<String>,
    pub expires: f64,
}

/// Result of importing cookies from a browser profile.
#[derive(Debug, Clone, Serialize)]
pub struct CookieImportResult {
    /// Successfully decrypted cookies ready for injection.
    pub cookies: Vec<SetCookie>,
    /// Which profile was used.
    pub profile_used: String,
    /// Total cookie rows found in DB matching domain (before decryption).
    pub found_in_db: usize,
    /// Names of cookies that failed decryption.
    pub decrypt_failed: Vec<String>,
    /// Count of cookies skipped because value was empty after decryption.
    pub empty_skipped: usize,
    /// All available profiles for this browser.
    pub available_profiles: Vec<String>,
}

/// Result of setting cookies via CDP.
#[derive(Debug, Clone, Serialize)]
pub struct CookieSetResult {
    /// Number of cookies successfully set.
    pub set: usize,
    /// Names of cookies that Chrome rejected.
    pub failed: Vec<String>,
}

/// RFC 6265 domain matching.
///
/// Matches `cookie_domain` against `filter_domain` with proper dot-boundary
/// checking. `.example.com` and `example.com` both match `sub.example.com`.
/// Rejects `badexample.com` matching `example.com`.
pub fn matches_domain(cookie_domain: &str, filter_domain: &str) -> bool {
    let cd = cookie_domain.trim_start_matches('.').to_lowercase();
    let fd = filter_domain.trim_start_matches('.').to_lowercase();

    if cd.is_empty() || fd.is_empty() {
        return false;
    }

    if cd == fd {
        return true;
    }
    // cookie_domain is a parent of filter_domain
    if fd.len() > cd.len() && fd.ends_with(&cd) && fd.as_bytes()[fd.len() - cd.len() - 1] == b'.' {
        return true;
    }
    // filter_domain is a parent of cookie_domain
    if cd.len() > fd.len() && cd.ends_with(&fd) && cd.as_bytes()[cd.len() - fd.len() - 1] == b'.' {
        return true;
    }
    false
}

impl SameSite {
    /// Parse from string (as stored in CookieInfo).
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "Strict" => Some(Self::Strict),
            "Lax" => Some(Self::Lax),
            "None" => Some(Self::None),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_exact_match() {
        assert!(matches_domain("example.com", "example.com"));
        assert!(matches_domain(".example.com", "example.com"));
    }

    #[test]
    fn domain_subdomain_match() {
        assert!(matches_domain(".example.com", "sub.example.com"));
        assert!(matches_domain("example.com", "sub.example.com"));
    }

    #[test]
    fn domain_rejects_substring() {
        assert!(!matches_domain("badexample.com", "example.com"));
        assert!(!matches_domain("example.com.evil.org", "example.com"));
        assert!(!matches_domain("example.com", "notexample.com"));
    }

    #[test]
    fn domain_empty_returns_false() {
        assert!(!matches_domain("", "example.com"));
        assert!(!matches_domain("example.com", ""));
        assert!(!matches_domain("", ""));
    }

    #[test]
    fn domain_case_insensitive() {
        assert!(matches_domain(".Example.COM", "sub.example.com"));
    }

    #[test]
    fn same_site_from_str() {
        assert!(matches!(SameSite::parse("Strict"), Some(SameSite::Strict)));
        assert!(matches!(SameSite::parse("Lax"), Some(SameSite::Lax)));
        assert!(matches!(SameSite::parse("None"), Some(SameSite::None)));
        assert!(SameSite::parse("invalid").is_none());
    }
}
