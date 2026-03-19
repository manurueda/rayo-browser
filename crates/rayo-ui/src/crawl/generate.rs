//! Generate test suites from flow graphs.
//!
//! Converts graph paths into YAML test suites, creating per-persona
//! journey tests and divergence verification tests.

use super::graph::*;
use crate::types::*;
use std::collections::{HashSet, VecDeque};

/// Generate test suites from a flow graph.
pub fn generate_from_graph(graph: &FlowGraph) -> Vec<(String, TestSuite)> {
    let mut suites = Vec::new();

    // 1. Per-persona journey tests: trace all paths from entry to leaves
    for persona in &graph.personas {
        if let Some(suite) = generate_persona_journey(graph, &persona.name) {
            let filename = format!(
                "flow-{}.test.yaml",
                persona.name.to_lowercase().replace(' ', "-")
            );
            suites.push((filename, suite));
        }
    }

    // 2. Divergence tests: verify persona-specific behavior at divergence points
    let divergence_suites = generate_divergence_tests(graph);
    suites.extend(divergence_suites);

    // 3. Smoke test: visit every node in BFS order
    let smoke = generate_smoke_from_graph(graph);
    suites.push(("flow-smoke.test.yaml".into(), smoke));

    suites
}

/// Generate a journey test for a single persona: BFS path through the graph.
fn generate_persona_journey(graph: &FlowGraph, persona_name: &str) -> Option<TestSuite> {
    // Find the entry node
    let entry_node = graph.nodes.iter().find(|n| n.depth == 0)?;

    // BFS through persona-accessible nodes
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut steps: Vec<TestStep> = Vec::new();

    queue.push_back(entry_node.id.clone());
    visited.insert(entry_node.id.clone());

    // Navigate to entry
    steps.push(TestStep {
        name: Some(format!("Navigate to {}", short_url(&entry_node.url))),
        navigate: Some(entry_node.url.clone()),
        assert: Some(vec![Assertion {
            page_map_contains: Some(PageMapAssertion {
                selector: None,
                text: None,
                role: None,
                tag: None,
            }),
            text_contains: None,
            screenshot: None,
            network_called: None,
        }]),
        ..Default::default()
    });

    while let Some(node_id) = queue.pop_front() {
        // Get persona-accessible outgoing edges
        let edges: Vec<&FlowEdge> = graph
            .outgoing_edges(&node_id)
            .into_iter()
            .filter(|e| e.personas.contains(&persona_name.to_string()))
            .collect();

        for edge in edges {
            if visited.contains(&edge.to_node) {
                continue;
            }
            visited.insert(edge.to_node.clone());

            if let Some(target_node) = graph.node(&edge.to_node) {
                // Add click/navigate step
                match &edge.action {
                    EdgeAction::Navigate { href } => {
                        steps.push(TestStep {
                            name: Some(format!(
                                "Follow '{}' to {}",
                                edge.label,
                                short_url(&target_node.url)
                            )),
                            navigate: Some(href.clone()),
                            assert: Some(vec![Assertion {
                                page_map_contains: Some(PageMapAssertion {
                                    selector: None,
                                    text: None,
                                    role: None,
                                    tag: None,
                                }),
                                text_contains: None,
                                screenshot: None,
                                network_called: None,
                            }]),
                            ..Default::default()
                        });
                    }
                    EdgeAction::Click { selector, text } => {
                        steps.push(TestStep {
                            name: Some(format!("Click '{text}'")),
                            click: Some(SelectorTarget::Selector(selector.clone())),
                            assert: Some(vec![Assertion {
                                page_map_contains: Some(PageMapAssertion {
                                    selector: None,
                                    text: None,
                                    role: None,
                                    tag: None,
                                }),
                                text_contains: None,
                                screenshot: None,
                                network_called: None,
                            }]),
                            ..Default::default()
                        });
                    }
                }

                queue.push_back(edge.to_node.clone());
            }
        }
    }

    if steps.len() <= 1 {
        return None; // Only has the initial navigate — not interesting
    }

    let setup = Vec::new();

    Some(TestSuite {
        name: format!("{persona_name} Journey"),
        viewport: None,
        setup,
        steps,
        teardown: Vec::new(),
    })
}

/// Generate tests that verify divergence behavior.
fn generate_divergence_tests(graph: &FlowGraph) -> Vec<(String, TestSuite)> {
    let mut suites = Vec::new();

    let divergence_nodes: Vec<&FlowNode> = graph.nodes.iter().filter(|n| n.is_divergence).collect();

    for node in divergence_nodes {
        let outgoing = graph.outgoing_edges(&node.id);
        if outgoing.is_empty() {
            continue;
        }

        // Group edges by persona
        let mut persona_edges: std::collections::HashMap<String, Vec<&FlowEdge>> =
            std::collections::HashMap::new();
        for edge in &outgoing {
            for persona in &edge.personas {
                persona_edges.entry(persona.clone()).or_default().push(edge);
            }
        }

        // Create a test that navigates to the divergence point and checks each persona's path
        for (persona_name, edges) in &persona_edges {
            let mut steps = Vec::new();

            // Navigate to divergence node
            steps.push(TestStep {
                name: Some(format!(
                    "Navigate to divergence point: {}",
                    short_url(&node.url)
                )),
                navigate: Some(node.url.clone()),
                ..Default::default()
            });

            // For each outgoing edge, assert destination
            for edge in edges {
                if let Some(target) = graph.node(&edge.to_node) {
                    steps.push(TestStep {
                        name: Some(format!(
                            "{persona_name}: '{}' → {} ({})",
                            edge.label,
                            short_url(&target.url),
                            target.page_type
                        )),
                        navigate: Some(target.url.clone()),
                        assert: Some(vec![Assertion {
                            text_contains: if !target.title.is_empty() {
                                Some(target.title.clone())
                            } else {
                                None
                            },
                            page_map_contains: Some(PageMapAssertion {
                                selector: None,
                                text: None,
                                role: None,
                                tag: None,
                            }),
                            screenshot: None,
                            network_called: None,
                        }]),
                        ..Default::default()
                    });
                }
            }

            if steps.len() > 1 {
                let slug = super::graph::node_id_from_url(&node.url);
                let filename = format!(
                    "flow-divergence-{}-{}.test.yaml",
                    &slug[..8.min(slug.len())],
                    persona_name.to_lowercase().replace(' ', "-")
                );
                suites.push((
                    filename,
                    TestSuite {
                        name: format!("Divergence: {} as {persona_name}", short_url(&node.url)),
                        viewport: None,
                        setup: Vec::new(),
                        steps,
                        teardown: Vec::new(),
                    },
                ));
            }
        }
    }

    suites
}

/// Generate a smoke test that visits every node.
fn generate_smoke_from_graph(graph: &FlowGraph) -> TestSuite {
    let steps: Vec<TestStep> = graph
        .nodes
        .iter()
        .filter(|n| n.page_type != PageType::Error && n.page_type != PageType::External)
        .map(|node| TestStep {
            name: Some(format!(
                "Load {} ({})",
                short_url(&node.url),
                node.page_type
            )),
            navigate: Some(node.url.clone()),
            assert: Some(vec![Assertion {
                page_map_contains: Some(PageMapAssertion {
                    selector: None,
                    text: None,
                    role: None,
                    tag: None,
                }),
                text_contains: None,
                screenshot: None,
                network_called: None,
            }]),
            ..Default::default()
        })
        .collect();

    TestSuite {
        name: "Flow Smoke Test — All Pages".to_string(),
        viewport: None,
        setup: Vec::new(),
        steps,
        teardown: Vec::new(),
    }
}

/// Shorten a URL for display (strip origin, keep path).
fn short_url(url: &str) -> String {
    url.find("://")
        .and_then(|i| url[i + 3..].find('/'))
        .map(|j| {
            let start = url.find("://").unwrap() + 3 + j;
            url[start..].to_string()
        })
        .unwrap_or_else(|| "/".to_string())
}

/// Default impl for TestStep (all fields None).
impl Default for TestStep {
    fn default() -> Self {
        Self {
            name: None,
            navigate: None,
            click: None,
            r#type: None,
            select: None,
            scroll: None,
            hover: None,
            press: None,
            wait: None,
            batch: None,
            cookie: None,
            assert: None,
        }
    }
}

/// Write generated test suites to disk.
pub fn write_flow_tests(
    suites: &[(String, TestSuite)],
    tests_dir: &std::path::Path,
    force: bool,
) -> Result<usize, crate::error::TestError> {
    std::fs::create_dir_all(tests_dir)?;

    let mut written = 0;
    for (filename, suite) in suites {
        let path = tests_dir.join(filename);
        if path.exists() && !force {
            continue;
        }

        let yaml = serde_yaml::to_string(suite)
            .map_err(|e| crate::error::TestError::Other(format!("YAML serialize error: {e}")))?;
        std::fs::write(&path, yaml)?;
        written += 1;
    }

    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    

    fn make_test_graph() -> FlowGraph {
        let home_id = node_id_from_url("http://localhost:3000/");
        let about_id = node_id_from_url("http://localhost:3000/about");
        let login_id = node_id_from_url("http://localhost:3000/login");

        FlowGraph {
            nodes: vec![
                FlowNode {
                    id: home_id.clone(),
                    url: "http://localhost:3000/".to_string(),
                    title: "Home".to_string(),
                    page_type: PageType::Landing,
                    headings: vec!["Welcome".to_string()],
                    interactive_count: 3,
                    personas: vec!["Anon".to_string(), "Pro".to_string()],
                    is_divergence: false,
                    depth: 0,
                    x: 0.0,
                    y: 0.0,
                },
                FlowNode {
                    id: about_id.clone(),
                    url: "http://localhost:3000/about".to_string(),
                    title: "About".to_string(),
                    page_type: PageType::Content,
                    headings: vec!["About Us".to_string()],
                    interactive_count: 1,
                    personas: vec!["Anon".to_string(), "Pro".to_string()],
                    is_divergence: false,
                    depth: 1,
                    x: 0.0,
                    y: 0.0,
                },
                FlowNode {
                    id: login_id.clone(),
                    url: "http://localhost:3000/login".to_string(),
                    title: "Login".to_string(),
                    page_type: PageType::Auth,
                    headings: vec!["Sign In".to_string()],
                    interactive_count: 3,
                    personas: vec!["Anon".to_string()],
                    is_divergence: false,
                    depth: 1,
                    x: 0.0,
                    y: 0.0,
                },
            ],
            edges: vec![
                FlowEdge {
                    id: edge_id(&home_id, &about_id, "About"),
                    from_node: home_id.clone(),
                    to_node: about_id.clone(),
                    action: EdgeAction::Navigate {
                        href: "http://localhost:3000/about".to_string(),
                    },
                    label: "About".to_string(),
                    personas: vec!["Anon".to_string(), "Pro".to_string()],
                    target_changed: false,
                },
                FlowEdge {
                    id: edge_id(&home_id, &login_id, "Login"),
                    from_node: home_id.clone(),
                    to_node: login_id.clone(),
                    action: EdgeAction::Navigate {
                        href: "http://localhost:3000/login".to_string(),
                    },
                    label: "Login".to_string(),
                    personas: vec!["Anon".to_string()],
                    target_changed: false,
                },
            ],
            personas: vec![
                PersonaRef {
                    name: "Anon".to_string(),
                    color: "#6b7280".to_string(),
                },
                PersonaRef {
                    name: "Pro".to_string(),
                    color: "#22c55e".to_string(),
                },
            ],
            entry_url: "http://localhost:3000/".to_string(),
            crawled_at: "2025-01-01T00:00:00Z".to_string(),
            stats: CrawlStats::default(),
        }
    }

    #[test]
    fn test_generate_creates_persona_journeys() {
        let graph = make_test_graph();
        let suites = generate_from_graph(&graph);

        // Should have: anon journey, pro journey, smoke test
        assert!(suites.len() >= 2);

        // Find anon journey
        let anon = suites
            .iter()
            .find(|(f, _)| f.contains("anonymous") || f.contains("anon"));
        assert!(anon.is_some(), "Should have anon journey");
    }

    #[test]
    fn test_generate_smoke_test() {
        let graph = make_test_graph();
        let suites = generate_from_graph(&graph);

        let smoke = suites
            .iter()
            .find(|(f, _)| f.contains("smoke"))
            .expect("Should have smoke test");

        assert_eq!(smoke.1.name, "Flow Smoke Test — All Pages");
        assert_eq!(smoke.1.steps.len(), 3); // home, about, login
    }

    #[test]
    fn test_short_url() {
        assert_eq!(short_url("http://localhost:3000/about"), "/about");
        assert_eq!(short_url("http://localhost:3000/"), "/");
        assert_eq!(short_url("http://localhost:3000"), "/");
    }
}
