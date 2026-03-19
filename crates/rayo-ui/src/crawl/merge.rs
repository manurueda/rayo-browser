//! Merge per-persona subgraphs into a unified flow graph.
//!
//! Union nodes by normalized URL, annotate edges with persona lists,
//! and detect divergence points where paths differ by persona.

use super::graph::*;
use std::collections::{HashMap, HashSet};

/// Merge multiple per-persona subgraphs into a unified FlowGraph.
pub fn merge_subgraphs(
    persona_graphs: Vec<PersonaGraphData>,
    personas: &[super::persona::Persona],
    entry_url: &str,
    duration_ms: u64,
) -> FlowGraph {
    let mut node_map: HashMap<String, FlowNode> = HashMap::new();
    let mut edge_map: HashMap<String, FlowEdge> = HashMap::new();

    // Merge nodes: union by ID, accumulate persona lists
    for (persona_name, nodes, edges) in &persona_graphs {
        for node in nodes {
            node_map
                .entry(node.id.clone())
                .and_modify(|existing| {
                    if !existing.personas.contains(persona_name) {
                        existing.personas.push(persona_name.clone());
                    }
                    // Keep the richer content (more headings, more elements)
                    if node.headings.len() > existing.headings.len() {
                        existing.headings = node.headings.clone();
                    }
                    if node.interactive_count > existing.interactive_count {
                        existing.interactive_count = node.interactive_count;
                    }
                    // Use minimum depth
                    if node.depth < existing.depth {
                        existing.depth = node.depth;
                    }
                })
                .or_insert_with(|| {
                    let mut n = node.clone();
                    n.personas = vec![persona_name.clone()];
                    n
                });
        }

        // Merge edges: union by (from, to, label), accumulate persona lists
        for edge in edges {
            edge_map
                .entry(edge.id.clone())
                .and_modify(|existing| {
                    if !existing.personas.contains(persona_name) {
                        existing.personas.push(persona_name.clone());
                    }
                })
                .or_insert_with(|| {
                    let mut e = edge.clone();
                    e.personas = vec![persona_name.clone()];
                    e
                });
        }
    }

    // Detect divergence points
    detect_divergences(&mut node_map, &edge_map, &persona_graphs);

    // Detect target-changed edges (same action from same source → different targets per persona)
    detect_target_changes(&mut edge_map, &persona_graphs);

    let mut nodes: Vec<FlowNode> = node_map.into_values().collect();
    let mut edges: Vec<FlowEdge> = edge_map.into_values().collect();

    // Sort for deterministic output
    nodes.sort_by(|a, b| a.depth.cmp(&b.depth).then(a.url.cmp(&b.url)));
    edges.sort_by(|a, b| a.from_node.cmp(&b.from_node).then(a.label.cmp(&b.label)));

    let divergence_points = nodes.iter().filter(|n| n.is_divergence).count();
    let max_depth = nodes.iter().map(|n| n.depth).max().unwrap_or(0);

    let persona_refs: Vec<PersonaRef> = personas
        .iter()
        .map(|p| PersonaRef {
            name: p.name.clone(),
            color: p.color.clone(),
        })
        .collect();

    FlowGraph {
        stats: CrawlStats {
            total_nodes: nodes.len(),
            total_edges: edges.len(),
            total_personas: persona_refs.len(),
            divergence_points,
            max_depth,
            duration_ms,
        },
        nodes,
        edges,
        personas: persona_refs,
        entry_url: entry_url.to_string(),
        crawled_at: chrono::Utc::now().to_rfc3339(),
    }
}

/// Detect divergence points: nodes where outgoing edges differ by persona.
fn detect_divergences(
    node_map: &mut HashMap<String, FlowNode>,
    edge_map: &HashMap<String, FlowEdge>,
    persona_graphs: &[PersonaGraphData],
) {
    // For each node that multiple personas share, check if outgoing edges differ
    for (node_id, node) in node_map.iter_mut() {
        if node.personas.len() <= 1 {
            continue;
        }

        // Collect outgoing edges per persona
        let mut edges_by_persona: HashMap<&str, HashSet<String>> = HashMap::new();
        for (persona_name, _, edges) in persona_graphs {
            let persona_edges: HashSet<String> = edges
                .iter()
                .filter(|e| e.from_node == *node_id)
                .map(|e| e.to_node.clone())
                .collect();
            if !persona_edges.is_empty() {
                edges_by_persona.insert(persona_name, persona_edges);
            }
        }

        // If any two personas have different outgoing target sets, it's a divergence
        let all_targets: Vec<&HashSet<String>> = edges_by_persona.values().collect();
        if all_targets.len() >= 2 {
            let first = all_targets[0];
            for other in &all_targets[1..] {
                if first != *other {
                    node.is_divergence = true;
                    break;
                }
            }
        }
    }

    // Also mark nodes with edges that only some personas can reach
    let all_persona_names: HashSet<&str> =
        persona_graphs.iter().map(|(n, _, _)| n.as_str()).collect();

    for edge in edge_map.values() {
        let edge_personas: HashSet<&str> = edge.personas.iter().map(|s| s.as_str()).collect();
        if edge_personas.len() < all_persona_names.len() {
            // This edge is persona-exclusive — mark source as divergence
            if let Some(node) = node_map.get_mut(&edge.from_node)
                && node.personas.len() > 1
            {
                node.is_divergence = true;
            }
        }
    }
}

/// Detect edges where the same action from the same source leads to different
/// targets depending on persona.
fn detect_target_changes(
    edge_map: &mut HashMap<String, FlowEdge>,
    persona_graphs: &[PersonaGraphData],
) {
    // Group edges by (from_node, label) across personas
    let mut action_groups: HashMap<(String, String), Vec<(String, String)>> = HashMap::new();

    for (persona_name, _, edges) in persona_graphs {
        for edge in edges {
            action_groups
                .entry((edge.from_node.clone(), edge.label.clone()))
                .or_default()
                .push((persona_name.clone(), edge.to_node.clone()));
        }
    }

    // If same (from, label) has different to_node for different personas → target_changed
    for ((from_node, label), targets) in &action_groups {
        let unique_targets: HashSet<&str> = targets.iter().map(|(_, t)| t.as_str()).collect();
        if unique_targets.len() > 1 {
            // Mark all edges matching this (from, label) as target_changed
            for edge in edge_map.values_mut() {
                if edge.from_node == *from_node && edge.label == *label {
                    edge.target_changed = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawl::persona::Persona;

    fn make_node(url: &str, personas: Vec<&str>, depth: usize) -> FlowNode {
        FlowNode {
            id: node_id_from_url(url),
            url: url.to_string(),
            title: "Page".to_string(),
            page_type: PageType::Content,
            headings: vec![],
            interactive_count: 0,
            personas: personas.into_iter().map(String::from).collect(),
            is_divergence: false,
            depth,
            x: 0.0,
            y: 0.0,
        }
    }

    fn make_edge(from_url: &str, to_url: &str, label: &str) -> FlowEdge {
        let from = node_id_from_url(from_url);
        let to = node_id_from_url(to_url);
        FlowEdge {
            id: edge_id(&from, &to, label),
            from_node: from,
            to_node: to,
            action: EdgeAction::Navigate {
                href: to_url.to_string(),
            },
            label: label.to_string(),
            personas: vec![],
            target_changed: false,
        }
    }

    fn make_personas() -> Vec<Persona> {
        vec![
            Persona {
                name: "Anon".to_string(),
                description: "".to_string(),
                color: "#aaa".to_string(),
                cookies: vec![],
                credentials: None,
                tags: vec![],
            },
            Persona {
                name: "Pro".to_string(),
                description: "".to_string(),
                color: "#bbb".to_string(),
                cookies: vec![],
                credentials: None,
                tags: vec![],
            },
        ]
    }

    #[test]
    fn test_merge_basic() {
        let home = "http://localhost:3000/";
        let about = "http://localhost:3000/about";

        let anon_nodes = vec![
            make_node(home, vec!["Anon"], 0),
            make_node(about, vec!["Anon"], 1),
        ];
        let anon_edges = vec![make_edge(home, about, "About")];

        let pro_nodes = vec![
            make_node(home, vec!["Pro"], 0),
            make_node(about, vec!["Pro"], 1),
        ];
        let pro_edges = vec![make_edge(home, about, "About")];

        let graph = merge_subgraphs(
            vec![
                ("Anon".to_string(), anon_nodes, anon_edges),
                ("Pro".to_string(), pro_nodes, pro_edges),
            ],
            &make_personas(),
            home,
            100,
        );

        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 1);

        // Both personas should be on each node
        let home_node = graph.node(&node_id_from_url(home)).unwrap();
        assert_eq!(home_node.personas.len(), 2);

        // Edge should have both personas
        assert_eq!(graph.edges[0].personas.len(), 2);
    }

    #[test]
    fn test_merge_detects_divergence() {
        let home = "http://localhost:3000/";
        let dashboard = "http://localhost:3000/dashboard";
        let paywall = "http://localhost:3000/paywall";

        // Anon: home → paywall
        let anon_nodes = vec![
            make_node(home, vec!["Anon"], 0),
            make_node(paywall, vec!["Anon"], 1),
        ];
        let anon_edges = vec![make_edge(home, paywall, "Get Started")];

        // Pro: home → dashboard
        let pro_nodes = vec![
            make_node(home, vec!["Pro"], 0),
            make_node(dashboard, vec!["Pro"], 1),
        ];
        let pro_edges = vec![make_edge(home, dashboard, "Get Started")];

        let graph = merge_subgraphs(
            vec![
                ("Anon".to_string(), anon_nodes, anon_edges),
                ("Pro".to_string(), pro_nodes, pro_edges),
            ],
            &make_personas(),
            home,
            100,
        );

        // Home should be a divergence point
        let home_node = graph.node(&node_id_from_url(home)).unwrap();
        assert!(home_node.is_divergence);

        // Should have 3 nodes total (home, paywall, dashboard)
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.stats.divergence_points, 1);
    }

    #[test]
    fn test_merge_target_changed() {
        let home = "http://localhost:3000/";
        let dash = "http://localhost:3000/dashboard";
        let pay = "http://localhost:3000/paywall";

        let anon_edges = vec![make_edge(home, pay, "Continue")];
        let pro_edges = vec![make_edge(home, dash, "Continue")];

        let graph = merge_subgraphs(
            vec![
                (
                    "Anon".to_string(),
                    vec![make_node(home, vec![], 0), make_node(pay, vec![], 1)],
                    anon_edges,
                ),
                (
                    "Pro".to_string(),
                    vec![make_node(home, vec![], 0), make_node(dash, vec![], 1)],
                    pro_edges,
                ),
            ],
            &make_personas(),
            home,
            50,
        );

        // Both edges should be marked target_changed since same label "Continue"
        // leads to different destinations
        let target_changed: Vec<_> = graph.edges.iter().filter(|e| e.target_changed).collect();
        assert_eq!(target_changed.len(), 2);
    }
}
