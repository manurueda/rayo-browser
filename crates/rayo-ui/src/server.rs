//! Axum web server — API + embedded dashboard UI, single binary, single port.

use crate::loader;
use crate::result::SuiteResult;
use crate::runner::{self, RunnerConfig, TestEvent};
use anyhow::Context;
use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{StatusCode, Uri, header},
    response::{Html, IntoResponse, Json, Response},
    routing::{get, post},
};
use rust_embed::Embed;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};
use tower_http::cors::CorsLayer;

/// Static UI files embedded at compile time.
/// Uses ui/out/ (full dashboard) if built, otherwise fallback-ui/ (placeholder).
#[derive(Embed)]
#[folder = "$RAYO_UI_ASSETS_DIR"]
struct UiAssets;

/// Shared state for the server.
struct AppState {
    tests_dir: PathBuf,
    baselines_dir: PathBuf,
    results: Mutex<Vec<SuiteResult>>,
    event_tx: broadcast::Sender<TestEvent>,
}

/// Start the web server with embedded dashboard.
pub async fn start_server(
    tests_dir: PathBuf,
    baselines_dir: PathBuf,
    port: u16,
    open_browser: bool,
) -> anyhow::Result<()> {
    let (event_tx, _) = broadcast::channel(256);

    let state = Arc::new(AppState {
        tests_dir,
        baselines_dir,
        results: Mutex::new(Vec::new()),
        event_tx,
    });

    let app = Router::new()
        // API routes
        .route("/api/suites", get(list_suites))
        .route("/api/results", get(list_results))
        .route("/api/run", post(run_suite))
        .route("/api/run/{name}", post(run_named_suite))
        .route("/ws/live", get(ws_handler))
        // Embedded UI — catch-all for everything else
        .fallback(get(serve_ui))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let url = format!("http://localhost:{port}");
    eprintln!("\n  ⚡ rayo-ui dashboard: {url}\n");

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

/// Serve embedded UI assets. Falls back to index.html for client-side routing.
async fn serve_ui(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // Try exact file match first
    if let Some(file) = UiAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CACHE_CONTROL, "public, max-age=3600")
            .body(axum::body::Body::from(file.data.to_vec()))
            .unwrap()
            .into_response();
    }

    // Try path.html (Next.js static export pattern)
    let html_path = format!("{path}.html");
    if let Some(file) = UiAssets::get(&html_path) {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(axum::body::Body::from(file.data.to_vec()))
            .unwrap()
            .into_response();
    }

    // Try path/index.html
    let index_path = if path.is_empty() {
        "index.html".to_string()
    } else {
        format!("{path}/index.html")
    };
    if let Some(file) = UiAssets::get(&index_path) {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(axum::body::Body::from(file.data.to_vec()))
            .unwrap()
            .into_response();
    }

    // Fall back to index.html for client-side routing
    if let Some(file) = UiAssets::get("index.html") {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(axum::body::Body::from(file.data.to_vec()))
            .unwrap()
            .into_response();
    }

    // No UI files embedded
    Html("rayo-ui server running. UI assets not found — rebuild with: cd ui && npm run build")
        .into_response()
}

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

async fn run_suite(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let files = match loader::load_suites(&state.tests_dir) {
        Ok(f) => f,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
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
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
                    .into_response();
            }
        }
    }

    let mut results = state.results.lock().await;
    results.extend(suite_results.clone());

    Json(serde_json::json!({ "results": suite_results })).into_response()
}

async fn run_named_suite(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> impl IntoResponse {
    let files = match loader::load_suites(&state.tests_dir) {
        Ok(f) => f,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let file = files.iter().find(|f| f.suite.name == name);
    let file = match file {
        Some(f) => f,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": format!("Suite '{name}' not found") })),
            )
                .into_response();
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
            Json(serde_json::json!({ "result": result })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.event_tx.subscribe();

    while let Ok(event) = rx.recv().await {
        let json = match serde_json::to_string(&event) {
            Ok(j) => j,
            Err(_) => continue,
        };
        if socket.send(Message::Text(json)).await.is_err() {
            break;
        }
    }
}
