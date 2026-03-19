# Flow Graph Crawler — Multi-Persona User Journey Mapping

## Problem

rayo-ui discovers flows per-page (auth, form, search, CRUD, navigation) but has no awareness of cross-page journeys. Real apps have branching paths: landing → login → paywall (free user) or dashboard (pro user). There's no way to see how different user types experience the same app, no graph visualization, and no persona-aware test generation.

## Solution

A BFS crawler that maps the entire app from the perspective of multiple user personas, builds a directed graph, renders it in an interactive Cytoscape.js dashboard, and generates persona-aware test suites.

## Modules

| Module | Responsibility |
|--------|---------------|
| `crawl/graph.rs` | FlowGraph, FlowNode, FlowEdge data structures + URL normalization + Cytoscape JSON export |
| `crawl/persona.rs` | Persona YAML loader + default persona generation + cookie conversion |
| `crawl/classifier.rs` | PageType classification from PageMap signals (URL keywords, elements, headings) |
| `crawl/mod.rs` | BFS crawler orchestration — per-persona crawl using rayo-core browser primitives |
| `crawl/merge.rs` | Merge per-persona subgraphs, divergence detection, target-change detection |
| `crawl/generate.rs` | FlowGraph → YAML test suites (journey tests, divergence tests, smoke tests) |
| `server.rs` (extended) | /flows page, /api/crawl, /api/flows routes + Cytoscape.js static assets |
| `templates/pages/flows.html` | Interactive graph dashboard with persona filtering + node sidebar |

## Boundaries

- Crawler consumes rayo-core browser primitives only (goto, page_map, set_cookies, click)
- No changes to rayo-core, rayo-mcp, rayo-visual, rayo-profiler, rayo-rules, rayo-updater
- Persona YAML format is independent — no coupling to test YAML format
- Graph JSON persistence in `.rayo/flows/` — separate from test data in `.rayo/tests/`
- Cytoscape.js + dagre vendored as static assets (same pattern as htmx)

## Verification

1. `cargo build --workspace` — compiles clean
2. `cargo test -p rayo-ui --lib` — 108 tests pass (includes new crawl module tests)
3. `cargo clippy --workspace` — 0 warnings
4. `cargo fmt --check --all` — clean
5. `rayo-ui crawl <url>` — CLI crawl works
6. `rayo-ui ui` → `/flows` — dashboard renders graph
