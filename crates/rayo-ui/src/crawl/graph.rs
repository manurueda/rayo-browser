//! Flow graph data structures for multi-persona user journey mapping.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Complete flow graph: all pages, transitions, and persona annotations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowGraph {
    /// All discovered pages.
    pub nodes: Vec<FlowNode>,
    /// All transitions between pages.
    pub edges: Vec<FlowEdge>,
    /// Personas used in this crawl.
    pub personas: Vec<PersonaRef>,
    /// Entry URL where crawl started.
    pub entry_url: String,
    /// When the crawl was performed (ISO 8601).
    pub crawled_at: String,
    /// Summary statistics.
    pub stats: CrawlStats,
}

/// A page in the flow graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowNode {
    /// Unique ID (hash of normalized URL).
    pub id: String,
    /// Page URL.
    pub url: String,
    /// Page title.
    pub title: String,
    /// Classification of the page.
    pub page_type: PageType,
    /// Headings found on the page.
    pub headings: Vec<String>,
    /// Number of interactive elements.
    pub interactive_count: usize,
    /// Which personas can reach this page.
    pub personas: Vec<String>,
    /// True if outgoing edges differ by persona.
    pub is_divergence: bool,
    /// BFS depth from entry URL.
    pub depth: usize,
    /// Layout X coordinate (set by visualization).
    #[serde(default)]
    pub x: f64,
    /// Layout Y coordinate (set by visualization).
    #[serde(default)]
    pub y: f64,
}

/// A transition between pages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowEdge {
    /// Unique edge ID.
    pub id: String,
    /// Source node ID.
    pub from_node: String,
    /// Destination node ID.
    pub to_node: String,
    /// How the transition happens.
    pub action: EdgeAction,
    /// Human-readable label (link text / button text).
    pub label: String,
    /// Which personas can take this transition.
    pub personas: Vec<String>,
    /// Same action leads to different destinations depending on persona.
    #[serde(default)]
    pub target_changed: bool,
}

/// How a transition between pages occurs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
pub enum EdgeAction {
    /// User clicks a link.
    #[serde(rename = "click")]
    Click { selector: String, text: String },
    /// Direct navigation (href follow).
    #[serde(rename = "navigate")]
    Navigate { href: String },
}

/// Page classification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Copy)]
#[serde(rename_all = "snake_case")]
pub enum PageType {
    Landing,
    Auth,
    Paywall,
    Dashboard,
    Settings,
    Content,
    Error,
    External,
}

impl PageType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Landing => "landing",
            Self::Auth => "auth",
            Self::Paywall => "paywall",
            Self::Dashboard => "dashboard",
            Self::Settings => "settings",
            Self::Content => "content",
            Self::Error => "error",
            Self::External => "external",
        }
    }

    /// Color used in visualization.
    pub fn color(&self) -> &str {
        match self {
            Self::Landing => "#3b82f6",   // blue
            Self::Auth => "#f59e0b",      // amber
            Self::Paywall => "#ef4444",   // red
            Self::Dashboard => "#22c55e", // green
            Self::Settings => "#8b5cf6",  // purple
            Self::Content => "#6b7280",   // gray
            Self::Error => "#f43f5e",     // rose
            Self::External => "#94a3b8",  // slate
        }
    }
}

impl fmt::Display for PageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Summary reference to a persona (stored in graph).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaRef {
    pub name: String,
    pub color: String,
}

/// Crawl statistics.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CrawlStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub total_personas: usize,
    pub divergence_points: usize,
    pub max_depth: usize,
    pub duration_ms: u64,
}

/// Result of crawling with a single persona.
#[derive(Debug, Clone)]
pub struct PersonaSubgraph {
    pub persona_name: String,
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
}

// ---------------------------------------------------------------------------
// URL normalization
// ---------------------------------------------------------------------------

/// Normalize a URL for deduplication: strip fragment, normalize trailing slash,
/// sort query params.
pub fn normalize_url(url: &str) -> String {
    // Strip fragment
    let url = url.split('#').next().unwrap_or(url);

    // Split query string
    let (base, query) = match url.split_once('?') {
        Some((b, q)) => (b, Some(q)),
        None => (url, None),
    };

    // Normalize trailing slash (keep root path like http://host:port/)
    let base = {
        let trimmed = base.trim_end_matches('/');
        // If trimming left us with just scheme://host, keep the trailing slash
        if trimmed.contains("://") && !trimmed[trimmed.find("://").unwrap() + 3..].contains('/') {
            base // Keep as-is (it's a root URL)
        } else if base.len() > 1 {
            trimmed
        } else {
            base
        }
    };

    // Sort query params
    match query {
        Some(q) if !q.is_empty() => {
            let mut params: Vec<&str> = q.split('&').collect();
            params.sort();
            format!("{base}?{}", params.join("&"))
        }
        _ => base.to_string(),
    }
}

/// Generate a node ID from a URL.
pub fn node_id_from_url(url: &str) -> String {
    let normalized = normalize_url(url);
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    normalized.hash(&mut hasher);
    format!("n{:016x}", hasher.finish())
}

/// Generate an edge ID from source, target, and action.
pub fn edge_id(from: &str, to: &str, label: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    from.hash(&mut hasher);
    to.hash(&mut hasher);
    label.hash(&mut hasher);
    format!("e{:016x}", hasher.finish())
}

/// Extract the origin (scheme + host + port) from a URL.
pub fn extract_origin(url: &str) -> Option<String> {
    let after_scheme = url.find("://").map(|i| i + 3)?;
    let rest = &url[after_scheme..];
    let end = rest.find('/').unwrap_or(rest.len());
    Some(url[..after_scheme + end].to_string())
}

/// Content hash for page deduplication (catches SPAs rendering same content at different URLs).
pub fn content_hash(title: &str, headings: &[String], interactive_count: usize) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    title.hash(&mut hasher);
    for h in headings {
        h.hash(&mut hasher);
    }
    interactive_count.hash(&mut hasher);
    hasher.finish()
}

/// Per-persona node/edge collections before merge.
pub type PersonaGraphData = (String, Vec<FlowNode>, Vec<FlowEdge>);

impl FlowGraph {
    /// Look up a node by ID.
    pub fn node(&self, id: &str) -> Option<&FlowNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Get edges originating from a node.
    pub fn outgoing_edges(&self, node_id: &str) -> Vec<&FlowEdge> {
        self.edges
            .iter()
            .filter(|e| e.from_node == node_id)
            .collect()
    }

    /// Get edges pointing to a node.
    pub fn incoming_edges(&self, node_id: &str) -> Vec<&FlowEdge> {
        self.edges.iter().filter(|e| e.to_node == node_id).collect()
    }

    /// Nodes reachable by only one persona.
    pub fn exclusive_nodes(&self) -> Vec<&FlowNode> {
        self.nodes
            .iter()
            .filter(|n| n.personas.len() == 1)
            .collect()
    }

    /// Convert graph to Cytoscape.js JSON elements format.
    pub fn to_cytoscape_elements(&self) -> serde_json::Value {
        let mut elements = Vec::new();

        for node in &self.nodes {
            elements.push(serde_json::json!({
                "group": "nodes",
                "data": {
                    "id": node.id,
                    "label": if node.title.is_empty() { &node.url } else { &node.title },
                    "url": node.url,
                    "pageType": node.page_type.as_str(),
                    "pageTypeColor": node.page_type.color(),
                    "interactiveCount": node.interactive_count,
                    "personas": node.personas,
                    "isDivergence": node.is_divergence,
                    "depth": node.depth,
                    "headings": node.headings,
                }
            }));
        }

        for edge in &self.edges {
            elements.push(serde_json::json!({
                "group": "edges",
                "data": {
                    "id": edge.id,
                    "source": edge.from_node,
                    "target": edge.to_node,
                    "label": edge.label,
                    "personas": edge.personas,
                    "targetChanged": edge.target_changed,
                }
            }));
        }

        serde_json::json!(elements)
    }

    /// Collect all unique persona names referenced in nodes/edges.
    pub fn referenced_personas(&self) -> Vec<String> {
        let mut seen = HashMap::new();
        for node in &self.nodes {
            for p in &node.personas {
                seen.entry(p.clone()).or_insert(());
            }
        }
        seen.into_keys().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_url_strips_fragment() {
        assert_eq!(
            normalize_url("http://localhost:3000/page#section"),
            "http://localhost:3000/page"
        );
    }

    #[test]
    fn test_normalize_url_strips_trailing_slash() {
        assert_eq!(
            normalize_url("http://localhost:3000/page/"),
            "http://localhost:3000/page"
        );
    }

    #[test]
    fn test_normalize_url_preserves_root() {
        assert_eq!(
            normalize_url("http://localhost:3000/"),
            "http://localhost:3000/"
        );
    }

    #[test]
    fn test_normalize_url_sorts_query_params() {
        assert_eq!(
            normalize_url("http://localhost:3000/page?b=2&a=1"),
            "http://localhost:3000/page?a=1&b=2"
        );
    }

    #[test]
    fn test_node_id_deterministic() {
        let id1 = node_id_from_url("http://localhost:3000/page");
        let id2 = node_id_from_url("http://localhost:3000/page");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_node_id_different_for_different_urls() {
        let id1 = node_id_from_url("http://localhost:3000/a");
        let id2 = node_id_from_url("http://localhost:3000/b");
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_extract_origin() {
        assert_eq!(
            extract_origin("http://localhost:3000/page/sub"),
            Some("http://localhost:3000".to_string())
        );
        assert_eq!(
            extract_origin("https://example.com"),
            Some("https://example.com".to_string())
        );
    }

    #[test]
    fn test_content_hash_same_content() {
        let h1 = content_hash("Title", &["H1".into()], 5);
        let h2 = content_hash("Title", &["H1".into()], 5);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_content_hash_different_content() {
        let h1 = content_hash("Title A", &["H1".into()], 5);
        let h2 = content_hash("Title B", &["H1".into()], 5);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_page_type_color() {
        assert_eq!(PageType::Landing.color(), "#3b82f6");
        assert_eq!(PageType::Error.color(), "#f43f5e");
    }
}
