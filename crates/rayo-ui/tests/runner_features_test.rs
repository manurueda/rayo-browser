//! Integration tests for content-aware waits, network capture/mocking,
//! and shared-page story execution.

use axum::response::Html;
use axum::routing::{get, post};
use axum::{Json, Router};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

#[path = "../src/error.rs"]
pub mod error;
#[path = "../src/result.rs"]
pub mod result;
#[path = "../src/runner.rs"]
pub mod runner;
#[path = "../src/types.rs"]
pub mod types;

pub mod story_types {
    use super::types::Assertion;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct UserStory {
        pub name: String,
        #[serde(default)]
        pub description: String,
        #[serde(default)]
        pub persona: Option<String>,
        #[serde(default)]
        pub importance: Option<String>,
        #[serde(default)]
        pub requires: Vec<String>,
        pub flows: Vec<StoryFlow>,
        #[serde(default)]
        pub tags: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct StoryFlow {
        pub name: String,
        #[serde(default)]
        pub then: Vec<StoryAssertion>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct StoryAssertion {
        pub description: String,
        #[serde(default)]
        pub assert: Option<Assertion>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct StoryResult {
        pub name: String,
        pub description: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub persona: Option<String>,
        pub importance: String,
        pub passed: bool,
        pub duration_ms: u64,
        pub flow_results: Vec<StoryFlowResult>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub error: Option<String>,
        pub started_at: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct StoryFlowResult {
        pub flow_name: String,
        pub passed: bool,
        pub duration_ms: u64,
        pub steps_passed: usize,
        pub steps_total: usize,
        pub then_results: Vec<StoryAssertionResult>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub error: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct StoryAssertionResult {
        pub description: String,
        pub passed: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
    }
}

#[path = "../src/story_runner.rs"]
pub mod story_runner;

use runner::RunnerConfig;
use story_runner::StoryRunnerConfig;
use story_types::{StoryAssertion, StoryFlow, UserStory};
use types::{Assertion, TestSuite};

fn chrome_available() -> bool {
    let paths = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium-browser",
        "/usr/bin/chromium",
    ];
    paths.iter().any(|p| std::path::Path::new(p).exists())
        || which::which("google-chrome").is_ok()
        || which::which("chromium").is_ok()
}

async fn start_feature_server() -> String {
    let app = Router::new()
        .route(
            "/waits",
            get(|| async {
                Html(
                    r#"<!doctype html>
                    <html>
                      <body>
                        <div id="status">Loading</div>
                        <script>
                          setTimeout(() => {
                            document.querySelector('#status').textContent = 'Loaded';
                            document.body.insertAdjacentText('beforeend', ' Ready');
                          }, 200);
                        </script>
                      </body>
                    </html>"#,
                )
            }),
        )
        .route(
            "/network",
            get(|| async {
                Html(
                    r#"<!doctype html>
                    <html>
                      <body>
                        <div id="users">Loading...</div>
                        <script>
                          fetch('/api/users', { method: 'POST' })
                            .then((response) => response.json())
                            .then((data) => {
                              document.querySelector('#users').textContent = data.users[0].name;
                              document.body.insertAdjacentText('beforeend', ' ' + data.users[0].name);
                            });
                        </script>
                      </body>
                    </html>"#,
                )
            }),
        )
        .route("/api/users", post(|| async { Json(serde_json::json!({ "users": [{ "name": "Real User" }] })) }))
        .route("/story/setup", get(|| async { Html("<h1>Setup Page</h1>") }))
        .route("/story/main", get(|| async { Html("<h1>Main Page</h1><button>Continue</button>") }))
        .route("/story/done", get(|| async { Html("<h1>Teardown Page</h1>") }));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind random port");
    let addr: SocketAddr = listener.local_addr().unwrap();
    let base_url = format!("http://127.0.0.1:{}", addr.port());

    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    base_url
}

fn temp_baselines_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rayo-ui-{test_name}-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[tokio::test]
async fn wait_text_and_element_text_work() {
    if !chrome_available() {
        eprintln!("SKIP: Chrome not available");
        return;
    }

    let base_url = start_feature_server().await;
    let yaml = r##"
name: Wait Feature Test
steps:
  - navigate: /waits
  - wait:
      text: Ready
      element_text:
        selector: "#status"
        contains: Loaded
      timeout_ms: 4000
    assert:
      - text_contains: Ready
"##;
    let suite: TestSuite = serde_yaml::from_str(yaml).unwrap();
    let config = RunnerConfig {
        baselines_dir: temp_baselines_dir("waits"),
        abort_on_failure: false,
        base_url: Some(base_url),
    };

    let result = runner::run_suite(&suite, &config, None)
        .await
        .expect("run_suite should succeed");

    assert!(
        result.pass,
        "wait.text and wait.element_text should pass: {:?}",
        result.steps
    );
}

#[tokio::test]
async fn network_mock_and_network_called_work() {
    if !chrome_available() {
        eprintln!("SKIP: Chrome not available");
        return;
    }

    let base_url = start_feature_server().await;
    let yaml = r##"
name: Network Feature Test
setup:
  - network_mock:
      url_pattern: "*/api/users*"
      response:
        status: 200
        body: '{"users":[{"name":"Mocked Ada"}]}'
        content_type: application/json
steps:
  - navigate: /network
  - wait:
      text: Mocked Ada
      element_text:
        selector: "#users"
        contains: Mocked Ada
      timeout_ms: 5000
    assert:
      - network_called:
          url: /api/users
          method: POST
      - text_contains: Mocked Ada
"##;
    let suite: TestSuite = serde_yaml::from_str(yaml).unwrap();
    let config = RunnerConfig {
        baselines_dir: temp_baselines_dir("network"),
        abort_on_failure: false,
        base_url: Some(base_url),
    };

    let result = runner::run_suite(&suite, &config, None)
        .await
        .expect("run_suite should succeed");

    assert!(
        result.pass,
        "network mock + network_called should pass: {:?}",
        result.steps
    );
}

#[tokio::test]
async fn shared_page_story_runs_setup_steps_and_teardown() {
    if !chrome_available() {
        eprintln!("SKIP: Chrome not available");
        return;
    }

    let base_url = start_feature_server().await;
    let suite_yaml = r#"
name: Story Flow
setup:
  - navigate: /story/setup
steps:
  - navigate: /story/main
teardown:
  - navigate: /story/done
"#;
    let suite: TestSuite = serde_yaml::from_str(suite_yaml).unwrap();
    let suites = HashMap::from([(suite.name.clone(), suite)]);
    let stories = vec![UserStory {
        name: "Story with full suite execution".into(),
        description: "Verifies shared-page story execution uses setup and teardown".into(),
        persona: None,
        importance: Some("high".into()),
        requires: Vec::new(),
        flows: vec![StoryFlow {
            name: "Story Flow".into(),
            then: vec![StoryAssertion {
                description: "Teardown page is visible".into(),
                assert: Some(Assertion {
                    page_map_contains: None,
                    text_contains: Some("Teardown Page".into()),
                    screenshot: None,
                    network_called: None,
                }),
            }],
        }],
        tags: Vec::new(),
    }];

    let results = story_runner::run_stories(
        &stories,
        &suites,
        &StoryRunnerConfig {
            baselines_dir: temp_baselines_dir("stories"),
            base_url: Some(base_url),
        },
    )
    .await;

    assert_eq!(results.len(), 1);
    let result = &results[0];
    assert!(result.passed, "story should pass: {:?}", result.error);
    assert_eq!(result.flow_results[0].steps_total, 3);
    assert_eq!(result.flow_results[0].steps_passed, 3);
    assert!(result.flow_results[0].then_results[0].passed);
}
