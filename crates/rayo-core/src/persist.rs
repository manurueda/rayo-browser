//! Cookie persistence for transparent auth.
//!
//! Saves and loads cookies per-domain to `~/.rayo/cookies/`.
//! This allows session cookies to survive across rayo restarts so agents
//! don't trigger re-auth on every conversation.

use crate::cookie::CookieInfo;
use std::path::{Path, PathBuf};

/// Directory where per-domain cookie files are stored.
fn cookies_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".rayo").join("cookies")
}

/// Sanitize a domain string for use as a filename.
///
/// Replaces characters that are not alphanumeric, dots, or hyphens with underscores.
fn sanitize_domain(domain: &str) -> String {
    domain
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Load persisted cookies for a domain.
///
/// Returns `None` if no cookie file exists, the file is unreadable, or all
/// cookies have expired.
pub fn load_domain_cookies(domain: &str) -> Option<Vec<CookieInfo>> {
    load_domain_cookies_from(&cookies_dir(), domain)
}

/// Persist cookies for a domain to disk.
///
/// Creates `~/.rayo/cookies/` if it doesn't exist. Overwrites any existing
/// cookie file for this domain.
pub fn save_domain_cookies(domain: &str, cookies: &[CookieInfo]) -> Result<(), std::io::Error> {
    save_domain_cookies_to(&cookies_dir(), domain, cookies)
}

/// Internal: load cookies from a specific directory.
fn load_domain_cookies_from(dir: &Path, domain: &str) -> Option<Vec<CookieInfo>> {
    let path = dir.join(format!("{}.json", sanitize_domain(domain)));
    let data = std::fs::read_to_string(&path).ok()?;
    let cookies: Vec<CookieInfo> = serde_json::from_str(&data).ok()?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);

    let valid: Vec<_> = cookies
        .into_iter()
        .filter(|c| {
            // expires <= 0 means session cookie -- keep it
            // expires > now means not yet expired -- keep it
            c.expires <= 0.0 || c.expires > now
        })
        .collect();

    if valid.is_empty() {
        // All cookies expired -- clean up the file
        let _ = std::fs::remove_file(&path);
        None
    } else {
        Some(valid)
    }
}

/// Internal: save cookies to a specific directory.
fn save_domain_cookies_to(
    dir: &Path,
    domain: &str,
    cookies: &[CookieInfo],
) -> Result<(), std::io::Error> {
    if cookies.is_empty() {
        return Ok(());
    }

    std::fs::create_dir_all(dir)?;

    let path = dir.join(format!("{}.json", sanitize_domain(domain)));
    let json = serde_json::to_string_pretty(cookies).map_err(std::io::Error::other)?;
    std::fs::write(&path, json)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_domain_basic() {
        assert_eq!(sanitize_domain("example.com"), "example.com");
        assert_eq!(sanitize_domain("sub.example.com"), "sub.example.com");
        assert_eq!(sanitize_domain("my-site.co.uk"), "my-site.co.uk");
    }

    #[test]
    fn sanitize_domain_special_chars() {
        assert_eq!(sanitize_domain("ex/ample"), "ex_ample");
        assert_eq!(sanitize_domain("ex..ample"), "ex..ample");
    }

    #[test]
    fn round_trip_cookies() {
        let dir = tempfile::tempdir().unwrap();

        let cookies = vec![CookieInfo {
            name: "session".to_string(),
            value: "abc123".to_string(),
            domain: ".example.com".to_string(),
            path: "/".to_string(),
            secure: true,
            http_only: true,
            same_site: Some("Lax".to_string()),
            expires: -1.0, // session cookie
        }];

        save_domain_cookies_to(dir.path(), "example.com", &cookies).unwrap();
        let loaded = load_domain_cookies_from(dir.path(), "example.com").unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "session");
        assert_eq!(loaded[0].value, "abc123");
    }

    #[test]
    fn load_nonexistent_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_domain_cookies_from(dir.path(), "nonexistent.example.com").is_none());
    }

    #[test]
    fn expired_cookies_are_filtered() {
        let dir = tempfile::tempdir().unwrap();

        let cookies = vec![CookieInfo {
            name: "old".to_string(),
            value: "expired".to_string(),
            domain: ".example.com".to_string(),
            path: "/".to_string(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: 1000.0, // Way in the past (1970)
        }];

        save_domain_cookies_to(dir.path(), "expired.com", &cookies).unwrap();
        assert!(load_domain_cookies_from(dir.path(), "expired.com").is_none());
    }

    #[test]
    fn session_cookies_are_preserved() {
        let dir = tempfile::tempdir().unwrap();

        let cookies = vec![CookieInfo {
            name: "sid".to_string(),
            value: "token123".to_string(),
            domain: ".example.com".to_string(),
            path: "/".to_string(),
            secure: true,
            http_only: true,
            same_site: None,
            expires: 0.0, // Session cookie
        }];

        save_domain_cookies_to(dir.path(), "example.com", &cookies).unwrap();
        let loaded = load_domain_cookies_from(dir.path(), "example.com").unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "sid");
    }

    #[test]
    fn future_cookies_are_preserved() {
        let dir = tempfile::tempdir().unwrap();

        let future_expiry = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
            + 86400.0; // 24 hours from now

        let cookies = vec![CookieInfo {
            name: "auth".to_string(),
            value: "valid".to_string(),
            domain: ".example.com".to_string(),
            path: "/".to_string(),
            secure: true,
            http_only: false,
            same_site: Some("Strict".to_string()),
            expires: future_expiry,
        }];

        save_domain_cookies_to(dir.path(), "example.com", &cookies).unwrap();
        let loaded = load_domain_cookies_from(dir.path(), "example.com").unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "auth");
    }

    #[test]
    fn empty_cookies_not_saved() {
        let dir = tempfile::tempdir().unwrap();
        save_domain_cookies_to(dir.path(), "example.com", &[]).unwrap();
        assert!(load_domain_cookies_from(dir.path(), "example.com").is_none());
    }
}
