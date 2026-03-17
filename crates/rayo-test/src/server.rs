//! Axum web server for the test UI — REST API + WebSocket.

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
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};
use tower_http::cors::CorsLayer;

/// Shared state for the server.
struct AppState {
    tests_dir: PathBuf,
    baselines_dir: PathBuf,
    results: Mutex<Vec<SuiteResult>>,
    event_tx: broadcast::Sender<TestEvent>,
}

/// Start the web server.
pub async fn start_server(
    tests_dir: PathBuf,
    baselines_dir: PathBuf,
    port: u16,
) -> anyhow::Result<()> {
    let (event_tx, _) = broadcast::channel(256);

    let state = Arc::new(AppState {
        tests_dir,
        baselines_dir,
        results: Mutex::new(Vec::new()),
        event_tx,
    });

    let app = Router::new()
        .route("/api/suites", get(list_suites))
        .route("/api/results", get(list_results))
        .route("/api/run", post(run_suite))
        .route("/api/run/{name}", post(run_named_suite))
        .route("/ws/live", get(ws_handler))
        .route("/", get(index_page))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("rayo-test server listening on http://localhost:{port}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context("Failed to bind server port")?;
    axum::serve(listener, app).await.context("Server error")?;
    Ok(())
}

async fn index_page() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
<html>
<head><title>rayo-test</title></head>
<body style="font-family: sans-serif; background: #0a0a0a; color: #e0e0e0; padding: 2rem;">
<h1>rayo-test server</h1>
<p>API endpoints:</p>
<ul>
<li>GET <a href="/api/suites">/api/suites</a> - List test suites</li>
<li>GET <a href="/api/results">/api/results</a> - List results</li>
<li>POST /api/run - Run all suites</li>
<li>POST /api/run/:name - Run a specific suite</li>
<li>WS /ws/live - Live test events</li>
</ul>
<p>Start the Next.js UI for the full experience.</p>
</body>
</html>"#,
    )
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
