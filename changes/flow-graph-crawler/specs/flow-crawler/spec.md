# Flow Crawler Capability

## ADDED Requirements

### REQ-CRAWL-001: BFS Flow Crawler
The system SHALL crawl a web application via BFS from an entry URL, visiting each reachable same-origin page, extracting page maps, classifying page types, and recording navigation transitions as a directed graph.

#### Scenario: Crawl a multi-page app
Given a web app running at http://localhost:3000 with pages /, /about, /login, /pricing
When the crawler runs with max_depth=2 and max_pages=20
Then it discovers all reachable pages and records transitions between them
And the result is a FlowGraph with nodes and edges

### REQ-CRAWL-002: Persona-Aware Crawling
The system SHALL support multiple user personas defined in YAML files, each with cookies and credentials, and crawl the app separately for each persona to capture persona-specific navigation paths.

#### Scenario: Two personas see different pages
Given an Anonymous persona with no cookies and a Pro persona with session cookies
When both personas crawl the same app
Then each persona's subgraph reflects only pages accessible to that persona
And the merged graph annotates each node and edge with the personas that can reach it

### REQ-CRAWL-003: Page Classification
The system SHALL classify each discovered page into one of: Landing, Auth, Paywall, Dashboard, Settings, Content, Error, External using weighted signals from URL keywords, interactive elements, headings, and text content.

#### Scenario: Classify a login page
Given a page at /login with password input and "Sign In" heading
When the classifier runs
Then the page is classified as Auth

### REQ-CRAWL-004: Divergence Detection
The system SHALL detect divergence points where different personas experience different outgoing transitions from the same page, marking those nodes in the graph.

#### Scenario: Detect paywall divergence
Given Anonymous clicks "Get Started" and lands on /paywall
And Pro User clicks "Get Started" and lands on /dashboard
When the graphs are merged
Then the source node is marked as a divergence point
And the edges are marked as target_changed

### REQ-CRAWL-005: Dashboard Visualization
The system SHALL render the flow graph in an interactive web dashboard at /flows using Cytoscape.js with dagre layout, supporting persona filtering, node click for details, and pan/zoom.

#### Scenario: Dashboard renders graph
Given a persisted flow graph with 5 nodes and 6 edges
When the user navigates to /flows
Then the graph renders with color-coded nodes by page type
And persona filter checkboxes are shown
And clicking a node opens a sidebar with page details

### REQ-CRAWL-006: Test Generation from Graph
The system SHALL generate YAML test suites from the flow graph including per-persona journey tests, divergence verification tests, and a smoke test visiting all nodes.

#### Scenario: Generate persona journey tests
Given a flow graph with 2 personas and 5 nodes
When test generation runs
Then it produces per-persona journey test files
And a smoke test file visiting all non-error pages

### REQ-CRAWL-007: CLI Subcommand
The system SHALL expose a `crawl` CLI subcommand with configurable URL, max_depth, max_pages, personas_dir, output_dir, and generate_tests flags.

#### Scenario: CLI crawl with defaults
Given no existing personas directory
When the user runs `rayo-ui crawl http://localhost:3000`
Then default personas (Anonymous + Authenticated) are created
And the crawl executes and saves a flow graph to .rayo/flows/

### REQ-CRAWL-008: Graph Persistence
The system SHALL persist the flow graph as JSON to .rayo/flows/flow-graph.json and reload it on dashboard startup.

#### Scenario: Graph survives server restart
Given a completed crawl with persisted graph
When the dashboard server restarts
Then the /flows page renders the previously crawled graph
