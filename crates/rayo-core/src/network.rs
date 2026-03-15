//! Network interception types for the rayo_network MCP tool.
//!
//! Provides capture, block, and mock capabilities via CDP Fetch domain.

use serde::{Deserialize, Serialize};

/// A captured network request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedRequest {
    pub url: String,
    pub method: String,
    pub resource_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<i64>,
    pub headers: Vec<(String, String)>,
    pub timestamp_ms: f64,
}

/// A rule for blocking network requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockRule {
    /// URL pattern (glob-style: * matches anything)
    pub url_pattern: String,
    /// Optional resource type filter (script, stylesheet, image, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
}

/// A rule for mocking network responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockRule {
    /// URL pattern to match
    pub url_pattern: String,
    /// HTTP status code to return
    #[serde(default = "default_status")]
    pub status: u16,
    /// Response body
    #[serde(default)]
    pub body: String,
    /// Response headers
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    /// Optional resource type filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
}

fn default_status() -> u16 {
    200
}

/// Network interceptor state.
/// Holds captured requests and active rules.
pub struct NetworkInterceptor {
    capturing: bool,
    captured: Vec<CapturedRequest>,
    block_rules: Vec<BlockRule>,
    mock_rules: Vec<MockRule>,
    max_captured: usize,
}

impl NetworkInterceptor {
    pub fn new() -> Self {
        Self {
            capturing: false,
            captured: Vec::new(),
            block_rules: Vec::new(),
            mock_rules: Vec::new(),
            max_captured: 1000,
        }
    }

    pub fn start_capture(&mut self) {
        self.capturing = true;
        self.captured.clear();
    }

    pub fn stop_capture(&mut self) {
        self.capturing = false;
    }

    pub fn is_capturing(&self) -> bool {
        self.capturing
    }

    pub fn record_request(&mut self, req: CapturedRequest) {
        if self.capturing && self.captured.len() < self.max_captured {
            self.captured.push(req);
        }
    }

    pub fn captured_requests(&self) -> &[CapturedRequest] {
        &self.captured
    }

    /// Get captured requests matching an optional URL pattern (simple contains match).
    pub fn filtered_requests(&self, url_pattern: Option<&str>) -> Vec<&CapturedRequest> {
        match url_pattern {
            Some(pattern) => self
                .captured
                .iter()
                .filter(|r| url_matches(&r.url, pattern))
                .collect(),
            None => self.captured.iter().collect(),
        }
    }

    pub fn add_block_rule(&mut self, rule: BlockRule) {
        self.block_rules.push(rule);
    }

    pub fn add_mock_rule(&mut self, rule: MockRule) {
        self.mock_rules.push(rule);
    }

    pub fn clear_rules(&mut self) {
        self.block_rules.clear();
        self.mock_rules.clear();
    }

    pub fn clear_all(&mut self) {
        self.capturing = false;
        self.captured.clear();
        self.block_rules.clear();
        self.mock_rules.clear();
    }

    /// Check if a request URL should be blocked.
    pub fn should_block(&self, url: &str, resource_type: Option<&str>) -> bool {
        self.block_rules.iter().any(|rule| {
            let url_match = url_matches(url, &rule.url_pattern);
            let type_match = match (&rule.resource_type, resource_type) {
                (Some(rt), Some(actual)) => rt.eq_ignore_ascii_case(actual),
                (Some(_), None) => false,
                (None, _) => true,
            };
            url_match && type_match
        })
    }

    /// Find a mock rule matching this request.
    pub fn find_mock(&self, url: &str, resource_type: Option<&str>) -> Option<&MockRule> {
        self.mock_rules.iter().find(|rule| {
            let url_match = url_matches(url, &rule.url_pattern);
            let type_match = match (&rule.resource_type, resource_type) {
                (Some(rt), Some(actual)) => rt.eq_ignore_ascii_case(actual),
                (Some(_), None) => false,
                (None, _) => true,
            };
            url_match && type_match
        })
    }

    pub fn has_active_rules(&self) -> bool {
        !self.block_rules.is_empty() || !self.mock_rules.is_empty()
    }

    pub fn block_rules(&self) -> &[BlockRule] {
        &self.block_rules
    }

    pub fn mock_rules(&self) -> &[MockRule] {
        &self.mock_rules
    }
}

impl Default for NetworkInterceptor {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple URL pattern matching.
/// Supports `*` as wildcard for any sequence of characters.
fn url_matches(url: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if pattern.contains('*') {
        let parts: Vec<&str> = pattern.split('*').collect();
        let mut pos = 0;
        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }
            match url[pos..].find(part) {
                Some(found) => {
                    if i == 0 && found != 0 {
                        return false; // First part must be at start
                    }
                    pos += found + part.len();
                }
                None => return false,
            }
        }
        // If pattern doesn't end with *, the last part must be at the end
        if !pattern.ends_with('*')
            && let Some(last) = parts.last()
            && !last.is_empty()
        {
            return url.ends_with(last);
        }
        true
    } else {
        url.contains(pattern)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_matches_wildcard() {
        assert!(url_matches(
            "https://example.com/api/v1/users",
            "*.com/api/*"
        ));
        assert!(url_matches("https://example.com/styles.css", "*.css"));
        assert!(!url_matches("https://example.com/app.js", "*.css"));
        assert!(url_matches("https://anything.com", "*"));
    }

    #[test]
    fn test_url_matches_contains() {
        assert!(url_matches("https://example.com/api/v1", "example.com"));
        assert!(!url_matches("https://other.com/api", "example.com"));
    }

    #[test]
    fn test_block_rule() {
        let mut interceptor = NetworkInterceptor::new();
        interceptor.add_block_rule(BlockRule {
            url_pattern: "*.analytics.com*".into(),
            resource_type: Some("script".into()),
        });

        assert!(interceptor.should_block("https://www.analytics.com/track.js", Some("script")));
        assert!(!interceptor.should_block("https://www.analytics.com/track.js", Some("image")));
        assert!(!interceptor.should_block("https://example.com/app.js", Some("script")));
    }

    #[test]
    fn test_mock_rule() {
        let mut interceptor = NetworkInterceptor::new();
        interceptor.add_mock_rule(MockRule {
            url_pattern: "*/api/users*".into(),
            status: 200,
            body: r#"[{"name":"test"}]"#.into(),
            headers: vec![("content-type".into(), "application/json".into())],
            resource_type: None,
        });

        let mock = interceptor.find_mock("https://example.com/api/users?page=1", None);
        assert!(mock.is_some());
        assert_eq!(mock.unwrap().status, 200);
    }

    #[test]
    fn test_capture() {
        let mut interceptor = NetworkInterceptor::new();
        interceptor.start_capture();
        interceptor.record_request(CapturedRequest {
            url: "https://example.com".into(),
            method: "GET".into(),
            resource_type: "document".into(),
            status: Some(200),
            headers: vec![],
            timestamp_ms: 0.0,
        });

        assert_eq!(interceptor.captured_requests().len(), 1);

        let filtered = interceptor.filtered_requests(Some("example"));
        assert_eq!(filtered.len(), 1);

        let filtered = interceptor.filtered_requests(Some("other.com"));
        assert_eq!(filtered.len(), 0);
    }
}
