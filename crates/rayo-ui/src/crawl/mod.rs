//! Flow graph crawler — BFS exploration of web apps per user persona.
//!
//! Crawls every reachable page from an entry URL, once per persona,
//! then merges results into a unified flow graph with divergence detection.

pub mod classifier;
pub mod generate;
pub mod graph;
pub mod merge;
pub mod persona;

use crate::error::TestError;
use graph::*;
use rayo_core::page_map::PageMap;
use rayo_core::{RayoBrowser, ViewportConfig};
use rayo_profiler::Profiler;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Configuration for the flow crawl.
#[derive(Debug, Clone)]
pub struct CrawlConfig {
    /// Entry URL (e.g., http://localhost:3000).
    pub url: String,
    /// Directory containing persona YAML files.
    pub personas_dir: PathBuf,
    /// Output directory for flow graph JSON.
    pub output_dir: PathBuf,
    /// Maximum BFS depth.
    pub max_depth: usize,
    /// Maximum pages per persona.
    pub max_pages: usize,
    /// Delay between navigations (ms).
    pub delay_ms: u64,
}

/// Result of the crawl process.
#[derive(Debug, Clone)]
pub struct CrawlResult {
    pub graph: FlowGraph,
    pub personas_used: usize,
    pub total_pages: usize,
    pub duration_ms: u64,
}

/// Crawl status for async tracking.
#[derive(Debug, Clone)]
pub enum CrawlStatus {
    Idle,
    Running {
        persona: String,
        pages_crawled: usize,
    },
    Complete {
        total_nodes: usize,
        total_edges: usize,
    },
    Failed {
        error: String,
    },
}

// ---------------------------------------------------------------------------
// Main crawl entry point
// ---------------------------------------------------------------------------

/// Crawl the app from all personas and produce a merged flow graph.
pub async fn crawl(config: CrawlConfig) -> Result<CrawlResult, TestError> {
    let start = Instant::now();

    // Load or create personas
    let mut personas = persona::load_personas(&config.personas_dir);
    if personas.is_empty() {
        println!("  No personas found, creating defaults...");
        let _ = persona::write_default_personas(&config.personas_dir);
        personas = persona::default_personas();
    }

    // Assign colors
    for (i, p) in personas.iter_mut().enumerate() {
        persona::assign_color(p, i);
    }

    println!(
        "\n  Flow Crawl: {} persona(s), max depth {}, max pages {}",
        personas.len(),
        config.max_depth,
        config.max_pages
    );

    // Launch browser
    let profiler = Profiler::new();
    let viewport = ViewportConfig::default();
    let browser = RayoBrowser::launch_with_config(profiler, viewport).await?;

    let base_url = config.url.trim_end_matches('/');
    let origin = extract_origin(base_url).unwrap_or_else(|| base_url.to_string());

    // Crawl each persona sequentially
    let mut persona_graphs: Vec<PersonaGraphData> = Vec::new();

    for persona in &personas {
        println!("\n  Persona: {} ({})", persona.name, persona.color);

        let page = browser.new_page().await?;

        // Inject persona cookies
        if !persona.cookies.is_empty() {
            let domain = extract_domain(base_url);
            let set_cookies = persona::to_set_cookies(&persona.cookies, domain.as_deref());
            if let Err(e) = page.set_cookies(set_cookies).await {
                println!("    Warning: failed to set cookies: {e}");
            }
        }

        // For "Authenticated" persona with no cookies, try auto-auth
        if persona.cookies.is_empty() && persona.tags.contains(&"authenticated".to_string()) {
            println!("    Attempting auto-auth via browser cookie import...");
            match page.goto_with_auto_auth(base_url, None).await {
                Ok(result) => match result.auto_auth {
                    rayo_core::AutoAuthStatus::Succeeded => {
                        println!("    Auto-auth: cookies imported successfully");
                    }
                    _ => {
                        println!("    Auto-auth: no auth wall detected (proceeding as-is)");
                    }
                },
                Err(e) => {
                    println!("    Auto-auth failed: {e}");
                }
            }
        }

        // BFS crawl
        let (nodes, edges) = bfs_crawl(
            &page,
            base_url,
            &origin,
            &persona.name,
            config.max_depth,
            config.max_pages,
            config.delay_ms,
        )
        .await;

        println!("    Crawled {} pages, {} edges", nodes.len(), edges.len());

        persona_graphs.push((persona.name.clone(), nodes, edges));

        // Clean up page (don't close browser — reuse for next persona)
        page.clear_cookies().await.ok();
    }

    // Close browser
    browser.close().await;

    // Merge
    let duration_ms = start.elapsed().as_millis() as u64;
    let graph = merge::merge_subgraphs(persona_graphs, &personas, base_url, duration_ms);

    println!(
        "\n  Merged: {} nodes, {} edges, {} divergence points",
        graph.stats.total_nodes, graph.stats.total_edges, graph.stats.divergence_points
    );

    // Persist
    persist_graph(&graph, &config.output_dir)?;

    Ok(CrawlResult {
        total_pages: graph.stats.total_nodes,
        personas_used: personas.len(),
        duration_ms,
        graph,
    })
}

// ---------------------------------------------------------------------------
// BFS crawler
// ---------------------------------------------------------------------------

/// BFS item: URL to visit with context.
struct BfsItem {
    url: String,
    depth: usize,
    parent_node_id: Option<String>,
    edge_label: Option<String>,
    edge_action: Option<EdgeAction>,
}

/// BFS crawl a single persona's view of the app.
async fn bfs_crawl(
    page: &rayo_core::RayoPage,
    entry_url: &str,
    origin: &str,
    persona_name: &str,
    max_depth: usize,
    max_pages: usize,
    delay_ms: u64,
) -> (Vec<FlowNode>, Vec<FlowEdge>) {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut content_hashes: HashMap<u64, String> = HashMap::new();
    let mut queue: VecDeque<BfsItem> = VecDeque::new();

    queue.push_back(BfsItem {
        url: entry_url.to_string(),
        depth: 0,
        parent_node_id: None,
        edge_label: None,
        edge_action: None,
    });

    while let Some(item) = queue.pop_front() {
        if nodes.len() >= max_pages {
            break;
        }
        if item.depth > max_depth {
            continue;
        }

        let normalized = normalize_url(&item.url);
        if visited.contains(&normalized) {
            // Still create edge to existing node if parent exists
            if let Some(parent_id) = &item.parent_node_id {
                let to_id = node_id_from_url(&item.url);
                let label = item.edge_label.as_deref().unwrap_or("link");
                let eid = edge_id(parent_id, &to_id, label);
                if !edges.iter().any(|e: &FlowEdge| e.id == eid) {
                    edges.push(FlowEdge {
                        id: eid,
                        from_node: parent_id.clone(),
                        to_node: to_id,
                        action: item.edge_action.unwrap_or(EdgeAction::Navigate {
                            href: item.url.clone(),
                        }),
                        label: label.to_string(),
                        personas: vec![persona_name.to_string()],
                        target_changed: false,
                    });
                }
            }
            continue;
        }
        visited.insert(normalized);

        // Navigate
        let progress = format!("[{}/{}]", nodes.len() + 1, max_pages);
        print!("    {progress} {}...", truncate_url(&item.url, 60));

        if let Err(e) = page.goto(&item.url).await {
            println!(" ERROR ({e})");
            let node = FlowNode {
                id: node_id_from_url(&item.url),
                url: item.url.clone(),
                title: "Error".to_string(),
                page_type: PageType::Error,
                headings: vec![],
                interactive_count: 0,
                personas: vec![persona_name.to_string()],
                is_divergence: false,
                depth: item.depth,
                x: 0.0,
                y: 0.0,
            };
            if let Some(parent_id) = &item.parent_node_id {
                let label = item.edge_label.as_deref().unwrap_or("link");
                edges.push(FlowEdge {
                    id: edge_id(parent_id, &node.id, label),
                    from_node: parent_id.clone(),
                    to_node: node.id.clone(),
                    action: item.edge_action.unwrap_or(EdgeAction::Navigate {
                        href: item.url.clone(),
                    }),
                    label: label.to_string(),
                    personas: vec![persona_name.to_string()],
                    target_changed: false,
                });
            }
            nodes.push(node);
            continue;
        }

        // Wait for page to settle
        let _ = page.wait_for_network_idle(300, 3000).await;

        // Get page map
        let page_map = match page.page_map(None).await {
            Ok(pm) => pm,
            Err(e) => {
                println!(" ERROR ({e})");
                continue;
            }
        };

        // Check current URL (may have redirected)
        let current_url = page.url().await.unwrap_or_else(|_| item.url.clone());
        let actual_url = if current_url.is_empty() {
            item.url.clone()
        } else {
            current_url
        };

        // Content hash dedup
        let c_hash = content_hash(
            &page_map.title,
            &page_map.headings,
            page_map.interactive.len(),
        );
        if let Some(existing_url) = content_hashes.get(&c_hash)
            && normalize_url(&actual_url) != normalize_url(existing_url)
        {
            // Same content at different URL — treat as duplicate
            println!(" (duplicate of {})", truncate_url(existing_url, 40));
            continue;
        }
        content_hashes.insert(c_hash, actual_url.clone());

        // Classify page
        let page_type = classifier::classify_page(&page_map, &actual_url);
        let title = page_map.title.clone();
        let headings = page_map.headings.clone();
        let interactive_count = page_map.interactive.len();

        println!(
            " {} ({} elements, {})",
            page_type,
            interactive_count,
            truncate_title(&title, 30)
        );

        // Create node
        let node_id = node_id_from_url(&actual_url);
        let node = FlowNode {
            id: node_id.clone(),
            url: actual_url.clone(),
            title: title.clone(),
            page_type,
            headings: headings.clone(),
            interactive_count,
            personas: vec![persona_name.to_string()],
            is_divergence: false,
            depth: item.depth,
            x: 0.0,
            y: 0.0,
        };
        nodes.push(node);

        // Create edge from parent
        if let Some(parent_id) = &item.parent_node_id {
            let label = item.edge_label.as_deref().unwrap_or("link");
            let eid = edge_id(parent_id, &node_id, label);
            if !edges.iter().any(|e| e.id == eid) {
                edges.push(FlowEdge {
                    id: eid,
                    from_node: parent_id.clone(),
                    to_node: node_id.clone(),
                    action: item.edge_action.unwrap_or(EdgeAction::Navigate {
                        href: actual_url.clone(),
                    }),
                    label: label.to_string(),
                    personas: vec![persona_name.to_string()],
                    target_changed: false,
                });
            }
        }

        // Extract navigation targets for next BFS level
        if item.depth < max_depth {
            let targets = extract_nav_targets(&page_map, origin);
            for target in targets {
                let target_normalized = normalize_url(&target.url);
                if !visited.contains(&target_normalized) {
                    queue.push_back(BfsItem {
                        url: target.url,
                        depth: item.depth + 1,
                        parent_node_id: Some(node_id.clone()),
                        edge_label: Some(target.label),
                        edge_action: Some(target.action),
                    });
                }
            }
        }

        // Rate limiting
        if delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }
    }

    (nodes, edges)
}

// ---------------------------------------------------------------------------
// Navigation target extraction
// ---------------------------------------------------------------------------

struct NavTarget {
    url: String,
    label: String,
    action: EdgeAction,
}

/// Extract same-origin navigation targets from a page map.
fn extract_nav_targets(page_map: &PageMap, origin: &str) -> Vec<NavTarget> {
    let mut targets = Vec::new();
    let mut seen_urls: HashSet<String> = HashSet::new();

    for el in &page_map.interactive {
        // Links with href
        if el.tag == "a"
            && let Some(href) = &el.href
        {
            let full_url = resolve_url(href, origin);
            if !is_same_origin(&full_url, origin) {
                continue;
            }
            if is_binary_url(&full_url) || is_special_protocol(href) {
                continue;
            }

            let normalized = normalize_url(&full_url);
            if seen_urls.contains(&normalized) {
                continue;
            }
            seen_urls.insert(normalized);

            let label = el.text.as_deref().unwrap_or(href.as_str()).to_string();
            let label = truncate_label(&label, 50);

            targets.push(NavTarget {
                url: full_url.clone(),
                label: label.clone(),
                action: EdgeAction::Navigate { href: full_url },
            });
        }

        // Buttons with navigation-like text
        if el.tag == "button"
            && let Some(text) = &el.text
            && is_navigation_button(text)
        {
            // We can't follow buttons without clicking — add as potential targets
            // with the button text as label. The actual URL will be determined
            // at click time in a future enhancement.
            let label = truncate_label(text, 50);
            targets.push(NavTarget {
                url: String::new(), // Will be filled by click exploration
                label: label.clone(),
                action: EdgeAction::Click {
                    selector: el.selector.clone(),
                    text: label,
                },
            });
        }
    }

    // Filter out empty-URL button targets for now (until click exploration is added)
    targets.retain(|t| !t.url.is_empty());

    targets
}

// ---------------------------------------------------------------------------
// URL helpers
// ---------------------------------------------------------------------------

/// Check if a URL is same-origin as the crawl base.
fn is_same_origin(url: &str, origin: &str) -> bool {
    url.starts_with(origin)
}

/// Check if URL points to a binary file.
fn is_binary_url(url: &str) -> bool {
    let path = url.split('?').next().unwrap_or(url);
    let extensions = [
        ".pdf", ".zip", ".tar", ".gz", ".png", ".jpg", ".jpeg", ".gif", ".svg", ".ico", ".mp4",
        ".mp3", ".wav", ".woff", ".woff2", ".ttf", ".eot", ".css", ".js", ".map",
    ];
    extensions
        .iter()
        .any(|ext| path.to_lowercase().ends_with(ext))
}

/// Check if href uses a special protocol (mailto, tel, javascript).
fn is_special_protocol(href: &str) -> bool {
    let lower = href.to_lowercase();
    lower.starts_with("mailto:")
        || lower.starts_with("tel:")
        || lower.starts_with("javascript:")
        || lower.starts_with("data:")
        || lower.starts_with("blob:")
}

/// Check if button text suggests navigation (not form submission).
fn is_navigation_button(text: &str) -> bool {
    let lower = text.to_lowercase().trim().to_string();
    let nav_words = [
        "dashboard",
        "settings",
        "home",
        "profile",
        "next",
        "continue",
        "get started",
        "go to",
        "view",
        "explore",
        "discover",
        "back",
        "menu",
    ];
    let skip_words = [
        "submit", "delete", "remove", "save", "cancel", "search", "send", "upload", "download",
        "close", "dismiss",
    ];

    if skip_words.iter().any(|w| lower.contains(w)) {
        return false;
    }
    nav_words.iter().any(|w| lower.contains(w))
}

/// Resolve a relative URL against an origin.
fn resolve_url(href: &str, origin: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        href.to_string()
    } else if href.starts_with('/') {
        format!("{origin}{href}")
    } else {
        format!("{}/{href}", origin.trim_end_matches('/'))
    }
}

/// Extract domain from a URL (for cookie injection).
fn extract_domain(url: &str) -> Option<String> {
    url.find("://").map(|i| &url[i + 3..]).map(|rest| {
        let end = rest.find('/').unwrap_or(rest.len());
        let host = &rest[..end];
        // Strip port
        host.split(':').next().unwrap_or(host).to_string()
    })
}

/// Truncate a URL for display.
fn truncate_url(url: &str, max_len: usize) -> String {
    if url.len() <= max_len {
        url.to_string()
    } else {
        format!("{}...", &url[..max_len - 3])
    }
}

/// Truncate a title for display.
fn truncate_title(title: &str, max_len: usize) -> &str {
    if title.len() <= max_len {
        title
    } else {
        &title[..max_len]
    }
}

/// Truncate a label for edge display.
fn truncate_label(label: &str, max_len: usize) -> String {
    let trimmed = label.trim();
    if trimmed.len() <= max_len {
        trimmed.to_string()
    } else {
        format!("{}...", &trimmed[..max_len - 3])
    }
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

/// Save flow graph to disk.
pub fn persist_graph(graph: &FlowGraph, output_dir: &Path) -> Result<(), TestError> {
    std::fs::create_dir_all(output_dir)?;
    let path = output_dir.join("flow-graph.json");
    let json = serde_json::to_string_pretty(graph)
        .map_err(|e| TestError::Other(format!("Failed to serialize flow graph: {e}")))?;
    std::fs::write(&path, json)?;
    println!("  Graph saved: {}", path.display());
    Ok(())
}

/// Load flow graph from disk.
pub fn load_graph(output_dir: &Path) -> Option<FlowGraph> {
    let path = output_dir.join("flow-graph.json");
    if !path.exists() {
        return None;
    }
    match std::fs::read_to_string(&path) {
        Ok(json) => match serde_json::from_str(&json) {
            Ok(graph) => Some(graph),
            Err(e) => {
                tracing::warn!("Failed to parse flow graph: {e}");
                None
            }
        },
        Err(e) => {
            tracing::warn!("Failed to read flow graph: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_binary_url() {
        assert!(is_binary_url("http://example.com/file.pdf"));
        assert!(is_binary_url("http://example.com/image.PNG"));
        assert!(!is_binary_url("http://example.com/page"));
        assert!(!is_binary_url("http://example.com/about"));
    }

    #[test]
    fn test_is_special_protocol() {
        assert!(is_special_protocol("mailto:test@example.com"));
        assert!(is_special_protocol("tel:555-1234"));
        assert!(is_special_protocol("javascript:void(0)"));
        assert!(!is_special_protocol("/about"));
        assert!(!is_special_protocol("http://example.com"));
    }

    #[test]
    fn test_is_navigation_button() {
        assert!(is_navigation_button("Dashboard"));
        assert!(is_navigation_button("Go to Settings"));
        assert!(is_navigation_button("Get Started"));
        assert!(!is_navigation_button("Submit"));
        assert!(!is_navigation_button("Delete Account"));
        assert!(!is_navigation_button("Search"));
    }

    #[test]
    fn test_resolve_url() {
        assert_eq!(
            resolve_url("/about", "http://localhost:3000"),
            "http://localhost:3000/about"
        );
        assert_eq!(
            resolve_url("https://other.com", "http://localhost:3000"),
            "https://other.com"
        );
        assert_eq!(
            resolve_url("page", "http://localhost:3000"),
            "http://localhost:3000/page"
        );
    }

    #[test]
    fn test_extract_domain() {
        assert_eq!(
            extract_domain("http://localhost:3000/page"),
            Some("localhost".to_string())
        );
        assert_eq!(
            extract_domain("https://example.com"),
            Some("example.com".to_string())
        );
    }

    #[test]
    fn test_truncate_label() {
        assert_eq!(truncate_label("Short", 50), "Short");
        assert_eq!(
            truncate_label("This is a very long label that should be truncated", 20),
            "This is a very lo..."
        );
    }
}
