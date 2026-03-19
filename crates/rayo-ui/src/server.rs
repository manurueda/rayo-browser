//! Axum web server — askama-rendered dashboard + JSON API, single binary, single port.

use crate::crawl;
use crate::loader;
use crate::result::SuiteResult;
use crate::runner::{self, RunnerConfig, TestEvent};
#[allow(unused_imports)]
use crate::templates::HtmlTemplate;
use crate::templates::*;
use anyhow::Context;
use askama::Template;
use axum::{
    Router,
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Json, Response},
    routing::{get, post},
};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, broadcast};
use tower_http::cors::CorsLayer;

// ---------------------------------------------------------------------------
// Static assets (vendored, compiled into binary)
// ---------------------------------------------------------------------------

const HTMX_JS: &str = include_str!("../static/htmx.min.js");
const HTMX_WS_JS: &str = include_str!("../static/htmx-ws.js");
const CYTOSCAPE_JS: &str = include_str!("../static/cytoscape.min.js");
const DAGRE_JS: &str = include_str!("../static/dagre.min.js");
const CYTOSCAPE_DAGRE_JS: &str = include_str!("../static/cytoscape-dagre.js");

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

struct AppState {
    tests_dir: PathBuf,
    baselines_dir: PathBuf,
    results: Mutex<Vec<SuiteResult>>,
    event_tx: broadcast::Sender<TestEvent>,
    discover_status: RwLock<DiscoverStatus>,
    flow_graph: RwLock<Option<crawl::graph::FlowGraph>>,
    crawl_status: RwLock<crawl::CrawlStatus>,
    flows_dir: PathBuf,
    personas_dir: PathBuf,
}

#[derive(Debug, Clone)]
enum DiscoverStatus {
    NotNeeded,
    Pending { chrome_available: bool },
    Running,
    Complete { tests_generated: usize },
    Failed { error: String },
}

// ---------------------------------------------------------------------------
// Server entry point
// ---------------------------------------------------------------------------

/// Start the web server with askama-rendered dashboard.
pub async fn start_server(
    tests_dir: PathBuf,
    baselines_dir: PathBuf,
    port: u16,
    open_browser: bool,
) -> anyhow::Result<()> {
    let (event_tx, _) = broadcast::channel(256);

    // Check if tests exist
    let has_tests = has_test_files(&tests_dir);
    let chrome_ok = chrome_available();

    let discover_status = if has_tests {
        DiscoverStatus::NotNeeded
    } else {
        DiscoverStatus::Pending {
            chrome_available: chrome_ok,
        }
    };

    let flows_dir = tests_dir.parent().unwrap_or(&tests_dir).join("flows");
    let personas_dir = tests_dir.parent().unwrap_or(&tests_dir).join("personas");

    // Load persisted flow graph if available
    let flow_graph = crawl::load_graph(&flows_dir);

    let state = Arc::new(AppState {
        tests_dir: tests_dir.clone(),
        baselines_dir: baselines_dir.clone(),
        results: Mutex::new(Vec::new()),
        event_tx: event_tx.clone(),
        discover_status: RwLock::new(discover_status),
        flow_graph: RwLock::new(flow_graph),
        crawl_status: RwLock::new(crawl::CrawlStatus::Idle),
        flows_dir,
        personas_dir,
    });

    // Auto-discover if no tests and Chrome is available
    if !has_tests && chrome_ok {
        let discover_state = state.clone();
        tokio::spawn(async move {
            auto_discover(discover_state).await;
        });
    }

    let app = Router::new()
        // Page routes (server-rendered HTML)
        .route("/", get(page_dashboard))
        .route("/suites", get(page_suites))
        .route("/suites/{name}", get(page_suite_detail))
        .route("/flows", get(page_flows))
        .route("/live", get(page_live))
        // Fragment routes (HTML partials for htmx)
        .route("/frag/stats", get(frag_stats))
        .route("/frag/results", get(frag_results))
        .route("/frag/suite-list", get(frag_suite_list))
        .route("/frag/available-suites", get(frag_available_suites))
        .route("/frag/flow-sidebar", get(frag_flow_sidebar))
        // JSON API routes
        .route("/api/suites", get(list_suites))
        .route("/api/results", get(list_results))
        .route("/api/run", post(run_all))
        .route("/api/run/{name}", post(run_named))
        .route("/api/discover/status", get(api_discover_status))
        .route("/api/flows", get(api_flow_graph))
        .route("/api/crawl", post(api_start_crawl))
        .route("/api/crawl/status", get(api_crawl_status))
        .route("/api/flows/generate-tests", post(api_generate_tests))
        // WebSocket
        .route("/ws/live", get(ws_handler))
        // Static assets
        .route("/static/htmx.min.js", get(serve_htmx))
        .route("/static/htmx-ws.js", get(serve_htmx_ws))
        .route("/static/cytoscape.min.js", get(serve_cytoscape))
        .route("/static/dagre.min.js", get(serve_dagre))
        .route("/static/cytoscape-dagre.js", get(serve_cytoscape_dagre))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let url = format!("http://localhost:{port}");
    eprintln!("\n  \u{26a1} rayo-ui dashboard: {url}\n");

    if open_browser {
        let _ = open::that(&url);
    }

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context("Failed to bind server port")?;
    axum::serve(listener, app).await.context("Server error")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Page handlers (full HTML)
// ---------------------------------------------------------------------------

async fn page_dashboard(State(state): State<Arc<AppState>>) -> Response {
    let has_tests = has_test_files(&state.tests_dir);

    if !has_tests {
        let ds = state.discover_status.read().await;
        let (chrome_available, discovering) = match *ds {
            DiscoverStatus::Pending { chrome_available } => (chrome_available, false),
            DiscoverStatus::Running => (true, true),
            DiscoverStatus::Complete { .. } => {
                // Discover just finished — reload to show dashboard
                drop(ds);
                // Fall through to show dashboard with discovered tests
                let results = state.results.lock().await;
                let suites = load_suite_summaries(&state.tests_dir);
                let stats = StatsData::from_results(&results);
                return HtmlTemplate(DashboardTemplate {
                    stats,
                    results: results.clone(),
                    suites,
                })
                .into_response();
            }
            _ => (false, false),
        };
        return HtmlTemplate(WelcomeTemplate {
            chrome_available,
            discovering,
            tests_dir: state.tests_dir.display().to_string(),
        })
        .into_response();
    }

    let results = state.results.lock().await;
    let suites = load_suite_summaries(&state.tests_dir);
    let stats = StatsData::from_results(&results);
    HtmlTemplate(DashboardTemplate {
        stats,
        results: results.clone(),
        suites,
    })
    .into_response()
}

async fn page_suites(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let results = state.results.lock().await;
    let suites = load_suite_summaries(&state.tests_dir);
    HtmlTemplate(SuitesTemplate {
        suites,
        results: results.clone(),
    })
}

async fn page_suite_detail(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Response {
    let results = state.results.lock().await;
    let result = results.iter().rev().find(|r| r.name == name);
    match result {
        Some(r) => HtmlTemplate(SuiteDetailTemplate { result: r.clone() }).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Html(format!(
                "Suite '{name}' not found. <a href=\"/suites\">Back to suites</a>"
            )),
        )
            .into_response(),
    }
}

async fn page_live() -> impl IntoResponse {
    HtmlTemplate(LiveTemplate)
}

// ---------------------------------------------------------------------------
// Fragment handlers (HTML partials for htmx)
// ---------------------------------------------------------------------------

async fn frag_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let results = state.results.lock().await;
    let stats = StatsData::from_results(&results);
    HtmlTemplate(StatsFragment { stats })
}

async fn frag_results(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let results = state.results.lock().await;
    HtmlTemplate(SuiteListFragment {
        results: results.clone(),
    })
}

async fn frag_suite_list(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let results = state.results.lock().await;
    HtmlTemplate(SuiteListFragment {
        results: results.clone(),
    })
}

async fn frag_available_suites(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let suites = load_suite_summaries(&state.tests_dir);
    HtmlTemplate(AvailableSuitesFragment { suites })
}

// ---------------------------------------------------------------------------
// JSON API handlers
// ---------------------------------------------------------------------------

async fn list_suites(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match loader::load_suites(&state.tests_dir) {
        Ok(files) => {
            let summaries: Vec<serde_json::Value> = files
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "name": f.suite.name,
                        "path": f.path.display().to_string(),
                        "steps": f.suite.steps.len(),
                        "has_setup": !f.suite.setup.is_empty(),
                        "has_teardown": !f.suite.teardown.is_empty(),
                    })
                })
                .collect();
            Json(serde_json::json!({ "suites": summaries })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn list_results(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let results = state.results.lock().await;
    Json(serde_json::json!({ "results": *results }))
}

async fn run_all(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let files = match loader::load_suites(&state.tests_dir) {
        Ok(f) => f,
        Err(e) => {
            return error_response(&headers, &e.to_string());
        }
    };

    let config = RunnerConfig {
        baselines_dir: state.baselines_dir.clone(),
        abort_on_failure: false,
    };

    let mut suite_results = Vec::new();
    for file in &files {
        match runner::run_suite(&file.suite, &config, Some(state.event_tx.clone())).await {
            Ok(result) => suite_results.push(result),
            Err(e) => {
                return error_response(&headers, &e.to_string());
            }
        }
    }

    let mut results = state.results.lock().await;
    results.extend(suite_results.clone());

    if is_htmx(&headers) {
        let stats = StatsData::from_results(&results);
        let stats_html = StatsFragment { stats }.render().unwrap_or_default();
        let list_html = SuiteListFragment {
            results: suite_results,
        }
        .render()
        .unwrap_or_default();
        Html(format!(
            "{list_html}\n<div id=\"stats\" hx-swap-oob=\"innerHTML\">{stats_html}</div>"
        ))
        .into_response()
    } else {
        Json(serde_json::json!({ "results": suite_results })).into_response()
    }
}

async fn run_named(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(name): axum::extract::Path<String>,
    headers: HeaderMap,
) -> Response {
    let files = match loader::load_suites(&state.tests_dir) {
        Ok(f) => f,
        Err(e) => {
            return error_response(&headers, &e.to_string());
        }
    };

    let file = match files.iter().find(|f| f.suite.name == name) {
        Some(f) => f,
        None => {
            return error_response(&headers, &format!("Suite '{name}' not found"));
        }
    };

    let config = RunnerConfig {
        baselines_dir: state.baselines_dir.clone(),
        abort_on_failure: false,
    };

    match runner::run_suite(&file.suite, &config, Some(state.event_tx.clone())).await {
        Ok(result) => {
            let mut results = state.results.lock().await;
            results.push(result.clone());

            if is_htmx(&headers) {
                HtmlTemplate(SuiteCardFragment { result }).into_response()
            } else {
                Json(serde_json::json!({ "result": result })).into_response()
            }
        }
        Err(e) => error_response(&headers, &e.to_string()),
    }
}

async fn api_discover_status(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let ds = state.discover_status.read().await;
    let json = match &*ds {
        DiscoverStatus::NotNeeded => serde_json::json!({"status": "not_needed"}),
        DiscoverStatus::Pending { chrome_available } => {
            serde_json::json!({"status": "pending", "chrome_available": chrome_available})
        }
        DiscoverStatus::Running => serde_json::json!({"status": "running"}),
        DiscoverStatus::Complete { tests_generated } => {
            serde_json::json!({"status": "complete", "tests_generated": tests_generated})
        }
        DiscoverStatus::Failed { error } => {
            serde_json::json!({"status": "failed", "error": error})
        }
    };
    Json(json)
}

// ---------------------------------------------------------------------------
// WebSocket handler — sends HTML fragments for htmx-ws
// ---------------------------------------------------------------------------

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.event_tx.subscribe();

    while let Ok(event) = rx.recv().await {
        let html = match &event {
            TestEvent::SuiteStarted { name, total_steps } => {
                let frag = LiveProgressFragment {
                    suite_name: name.clone(),
                    current: 0,
                    total: *total_steps,
                    percent: 0.0,
                };
                frag.render().unwrap_or_default()
            }
            TestEvent::StepCompleted { result, index } => {
                let event_frag = LiveEventFragment {
                    step: result.clone(),
                    index: *index,
                };
                let progress_frag = LiveProgressFragment {
                    suite_name: String::new(),
                    current: index + 1,
                    total: 0,
                    percent: 0.0,
                };
                let event_html = event_frag.render().unwrap_or_default();
                let progress_html = progress_frag.render().unwrap_or_default();
                format!("{event_html}\n{progress_html}")
            }
            TestEvent::SuiteCompleted { result } => {
                let status = if result.pass { "PASSED" } else { "FAILED" };
                let color = if result.pass {
                    "text-green-500"
                } else {
                    "text-red-500"
                };
                format!(
                    r#"<div id="live-status" hx-swap-oob="innerHTML"><span class="inline-flex items-center gap-2 text-sm"><span class="w-2 h-2 bg-green-500 rounded-full"></span><span class="{color} font-medium">{} {status}</span></span></div>"#,
                    result.name
                )
            }
            _ => continue,
        };

        if socket.send(Message::Text(html.into())).await.is_err() {
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// Flow graph page + API handlers
// ---------------------------------------------------------------------------

async fn page_flows(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let graph = state.flow_graph.read().await;
    let crawl_status = state.crawl_status.read().await;
    let is_crawling = matches!(*crawl_status, crawl::CrawlStatus::Running { .. });
    HtmlTemplate(FlowsTemplate::from_graph(&graph, is_crawling))
}

#[derive(Deserialize)]
struct FlowSidebarQuery {
    node: String,
}

async fn frag_flow_sidebar(
    State(state): State<Arc<AppState>>,
    Query(query): Query<FlowSidebarQuery>,
) -> Response {
    let graph = state.flow_graph.read().await;
    if let Some(ref graph) = *graph
        && let Some(node) = graph.node(&query.node)
    {
        let outgoing = graph.outgoing_edges(&query.node);
        let incoming = graph.incoming_edges(&query.node);
        return HtmlTemplate(FlowSidebarFragment {
            node: node.clone(),
            outgoing_edges: outgoing.into_iter().cloned().collect(),
            incoming_edges: incoming.into_iter().cloned().collect(),
            graph_personas: graph.personas.clone(),
        })
        .into_response();
    }
    (StatusCode::NOT_FOUND, Html("Node not found".to_string())).into_response()
}

async fn api_flow_graph(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let graph = state.flow_graph.read().await;
    match &*graph {
        Some(g) => Json(serde_json::json!({
            "graph": g,
            "elements": g.to_cytoscape_elements(),
        })),
        None => Json(serde_json::json!({"graph": null})),
    }
}

#[derive(Deserialize)]
struct CrawlParams {
    url: Option<String>,
    max_depth: Option<usize>,
    max_pages: Option<usize>,
}

async fn api_start_crawl(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(params): Json<CrawlParams>,
) -> Response {
    // Check if already crawling
    {
        let status = state.crawl_status.read().await;
        if matches!(*status, crawl::CrawlStatus::Running { .. }) {
            return error_response(&headers, "Crawl already in progress");
        }
    }

    let url = params.url.unwrap_or_else(|| {
        // Try to detect target URL
        "http://localhost:3000".to_string()
    });

    let config = crawl::CrawlConfig {
        url: url.clone(),
        personas_dir: state.personas_dir.clone(),
        output_dir: state.flows_dir.clone(),
        max_depth: params.max_depth.unwrap_or(5),
        max_pages: params.max_pages.unwrap_or(100),
        delay_ms: 100,
    };

    *state.crawl_status.write().await = crawl::CrawlStatus::Running {
        persona: "Starting...".to_string(),
        pages_crawled: 0,
    };

    let crawl_state = state.clone();
    tokio::spawn(async move {
        match crawl::crawl(config).await {
            Ok(result) => {
                *crawl_state.flow_graph.write().await = Some(result.graph.clone());
                *crawl_state.crawl_status.write().await = crawl::CrawlStatus::Complete {
                    total_nodes: result.graph.stats.total_nodes,
                    total_edges: result.graph.stats.total_edges,
                };
            }
            Err(e) => {
                *crawl_state.crawl_status.write().await = crawl::CrawlStatus::Failed {
                    error: e.to_string(),
                };
            }
        }
    });

    if is_htmx(&headers) {
        Html(r#"<div class="text-sm text-rayo-400">Crawl started... <span class="htmx-indicator">&#8987;</span></div>"#.to_string()).into_response()
    } else {
        Json(serde_json::json!({"status": "started"})).into_response()
    }
}

async fn api_crawl_status(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let status = state.crawl_status.read().await;
    let json = match &*status {
        crawl::CrawlStatus::Idle => serde_json::json!({"status": "idle"}),
        crawl::CrawlStatus::Running {
            persona,
            pages_crawled,
        } => serde_json::json!({
            "status": "running",
            "persona": persona,
            "pages_crawled": pages_crawled,
        }),
        crawl::CrawlStatus::Complete {
            total_nodes,
            total_edges,
        } => serde_json::json!({
            "status": "complete",
            "total_nodes": total_nodes,
            "total_edges": total_edges,
        }),
        crawl::CrawlStatus::Failed { error } => {
            serde_json::json!({"status": "failed", "error": error})
        }
    };
    Json(json)
}

async fn api_generate_tests(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let graph = state.flow_graph.read().await;
    match &*graph {
        Some(g) => {
            let suites = crawl::generate::generate_from_graph(g);
            match crawl::generate::write_flow_tests(&suites, &state.tests_dir, false) {
                Ok(written) => {
                    if is_htmx(&headers) {
                        Html(format!(
                            r#"<div class="text-sm text-green-500">Generated {written} test file(s)</div>"#
                        ))
                        .into_response()
                    } else {
                        Json(serde_json::json!({"tests_written": written})).into_response()
                    }
                }
                Err(e) => error_response(&headers, &e.to_string()),
            }
        }
        None => error_response(&headers, "No flow graph available. Run a crawl first."),
    }
}

// ---------------------------------------------------------------------------
// Static asset handlers
// ---------------------------------------------------------------------------

async fn serve_htmx() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/javascript"),
            (header::CACHE_CONTROL, "public, max-age=31536000"),
        ],
        HTMX_JS,
    )
}

async fn serve_htmx_ws() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/javascript"),
            (header::CACHE_CONTROL, "public, max-age=31536000"),
        ],
        HTMX_WS_JS,
    )
}

async fn serve_cytoscape() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/javascript"),
            (header::CACHE_CONTROL, "public, max-age=31536000"),
        ],
        CYTOSCAPE_JS,
    )
}

async fn serve_dagre() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/javascript"),
            (header::CACHE_CONTROL, "public, max-age=31536000"),
        ],
        DAGRE_JS,
    )
}

async fn serve_cytoscape_dagre() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/javascript"),
            (header::CACHE_CONTROL, "public, max-age=31536000"),
        ],
        CYTOSCAPE_DAGRE_JS,
    )
}

// ---------------------------------------------------------------------------
// Auto-discover
// ---------------------------------------------------------------------------

async fn auto_discover(state: Arc<AppState>) {
    // Create directories
    let _ = std::fs::create_dir_all(&state.tests_dir);
    let _ = std::fs::create_dir_all(&state.baselines_dir);

    *state.discover_status.write().await = DiscoverStatus::Running;

    // Detect target URL by probing common ports
    let url = detect_target_url().await;

    let config = crate::discover::DiscoverConfig {
        url,
        project_dir: PathBuf::from("."),
        tests_dir: state.tests_dir.clone(),
        baselines_dir: state.baselines_dir.clone(),
        diff_mode: false,
        force: false,
        max_pages: 50,
    };

    match crate::discover::discover(config).await {
        Ok(result) => {
            *state.discover_status.write().await = DiscoverStatus::Complete {
                tests_generated: result.tests_generated,
            };
            eprintln!(
                "  \u{26a1} Auto-discover: {} tests generated (health: {}%)",
                result.tests_generated, result.health_score
            );
        }
        Err(e) => {
            *state.discover_status.write().await = DiscoverStatus::Failed {
                error: e.to_string(),
            };
            eprintln!("  Auto-discover failed: {e}");
        }
    }
}

/// Probe common dev server ports and return the first responding URL.
async fn detect_target_url() -> String {
    let ports = [3000, 5173, 8080, 4000, 8000, 3001];
    for port in ports {
        let url = format!("http://localhost:{port}");
        // Try to connect with a short timeout
        match tokio::time::timeout(
            std::time::Duration::from_millis(500),
            tokio::net::TcpStream::connect(format!("127.0.0.1:{port}")),
        )
        .await
        {
            Ok(Ok(_)) => return url,
            _ => continue,
        }
    }
    // Default fallback
    "http://localhost:3000".to_string()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn has_test_files(tests_dir: &std::path::Path) -> bool {
    if !tests_dir.exists() {
        return false;
    }
    let pattern = tests_dir.join("*.test.yaml");
    glob::glob(pattern.to_str().unwrap_or(""))
        .map(|paths| paths.count() > 0)
        .unwrap_or(false)
}

fn chrome_available() -> bool {
    // Check common Chrome/Chromium paths
    if which::which("google-chrome").is_ok()
        || which::which("chromium").is_ok()
        || which::which("chromium-browser").is_ok()
    {
        return true;
    }

    // macOS app bundles
    let mac_paths = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
    ];
    for path in &mac_paths {
        if std::path::Path::new(path).exists() {
            return true;
        }
    }

    // Check playwright cache (used by rayo-core)
    if let Ok(home) = std::env::var("HOME") {
        let playwright_chrome = format!(
            "{home}/Library/Caches/ms-playwright/chromium_headless_shell-1208/chrome-headless-shell-mac-arm64/chrome-headless-shell"
        );
        if std::path::Path::new(&playwright_chrome).exists() {
            return true;
        }
    }

    false
}

fn load_suite_summaries(tests_dir: &std::path::Path) -> Vec<SuiteSummary> {
    match loader::load_suites(tests_dir) {
        Ok(files) => files
            .iter()
            .map(|f| SuiteSummary {
                name: f.suite.name.clone(),
                path: f.path.display().to_string(),
                steps: f.suite.steps.len(),
                has_setup: !f.suite.setup.is_empty(),
                has_teardown: !f.suite.teardown.is_empty(),
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn is_htmx(headers: &HeaderMap) -> bool {
    headers.contains_key("hx-request")
}

fn error_response(headers: &HeaderMap, msg: &str) -> Response {
    if is_htmx(headers) {
        let frag = ErrorFragment {
            message: msg.to_string(),
        };
        (StatusCode::INTERNAL_SERVER_ERROR, HtmlTemplate(frag)).into_response()
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": msg })),
        )
            .into_response()
    }
}
