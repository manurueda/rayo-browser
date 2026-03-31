//! Test runner — executes test suites against a browser.

use crate::error::TestError;
use crate::result::{AssertionResult, StepResult, SuiteResult};
use crate::types::*;
use rayo_core::batch::{ActionTarget, BatchAction};
use rayo_core::network::{CapturedRequest, MockRule, NetworkInterceptor};
use rayo_core::{RayoBrowser, RayoPage, ViewportConfig};
use rayo_profiler::Profiler;
use rayo_visual::BaselineManager;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, broadcast};

/// Event emitted during test execution (for live UI updates).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub enum TestEvent {
    SuiteStarted { name: String, total_steps: usize },
    StepStarted { name: String, index: usize },
    StepCompleted { result: StepResult, index: usize },
    SuiteCompleted { result: SuiteResult },
}

/// Configuration for the test runner.
pub struct RunnerConfig {
    pub baselines_dir: PathBuf,
    pub abort_on_failure: bool,
    /// Base URL prepended to relative navigate paths (e.g. `http://localhost:3000`).
    pub base_url: Option<String>,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            baselines_dir: PathBuf::from(".rayo/baselines"),
            abort_on_failure: false,
            base_url: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RunnerRequirements {
    pub network_capture: bool,
    pub network_interception: bool,
}

impl RunnerRequirements {
    pub(crate) fn merge(&mut self, other: Self) {
        self.network_capture |= other.network_capture;
        self.network_interception |= other.network_interception;
    }

    fn is_enabled(self) -> bool {
        self.network_capture || self.network_interception
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NetworkMode {
    Disabled,
    Monitoring,
    Interception,
}

pub(crate) struct RunnerContext {
    network: Option<Arc<Mutex<NetworkInterceptor>>>,
    network_mode: NetworkMode,
    network_ready: bool,
}

impl RunnerContext {
    pub(crate) fn new(requirements: RunnerRequirements) -> Self {
        let network_mode = if requirements.network_interception {
            NetworkMode::Interception
        } else if requirements.network_capture {
            NetworkMode::Monitoring
        } else {
            NetworkMode::Disabled
        };

        Self {
            network: requirements
                .is_enabled()
                .then(|| Arc::new(Mutex::new(NetworkInterceptor::new()))),
            network_mode,
            network_ready: false,
        }
    }

    async fn ensure_network_ready(&mut self, page: &RayoPage) -> Result<(), TestError> {
        if self.network_ready {
            return Ok(());
        }

        let Some(network) = &self.network else {
            return Ok(());
        };

        match self.network_mode {
            NetworkMode::Disabled => return Ok(()),
            NetworkMode::Monitoring => page.enable_network_monitoring(Arc::clone(network)).await?,
            NetworkMode::Interception => {
                page.enable_network_interception(Arc::clone(network))
                    .await?
            }
        }

        self.network_ready = true;
        Ok(())
    }

    async fn begin_scope(&mut self, page: &RayoPage) -> Result<(), TestError> {
        self.ensure_network_ready(page).await?;

        if let Some(network) = &self.network {
            let mut network = network.lock().await;
            network.clear_rules();
            network.start_capture();
        }

        Ok(())
    }

    async fn end_scope(&self) {
        if let Some(network) = &self.network {
            let mut network = network.lock().await;
            network.stop_capture();
            network.clear_rules();
        }
    }

    async fn add_mock_rule(
        &mut self,
        page: &RayoPage,
        action: &NetworkMockAction,
    ) -> Result<(), TestError> {
        if self.network_mode == NetworkMode::Disabled {
            self.network_mode = NetworkMode::Interception;
            self.network = Some(Arc::new(Mutex::new(NetworkInterceptor::new())));
        } else if self.network_mode == NetworkMode::Monitoring && !self.network_ready {
            self.network_mode = NetworkMode::Interception;
        }

        self.ensure_network_ready(page).await?;

        let Some(network) = &self.network else {
            return Err(TestError::Other(
                "network mock requested but no network interceptor is available".into(),
            ));
        };

        let mut headers: Vec<(String, String)> = action
            .response
            .headers
            .clone()
            .unwrap_or_default()
            .into_iter()
            .collect();

        if let Some(content_type) = &action.response.content_type {
            let has_content_type = headers
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case("content-type"));
            if !has_content_type {
                headers.push(("content-type".into(), content_type.clone()));
            }
        }

        network.lock().await.add_mock_rule(MockRule {
            url_pattern: action.url_pattern.clone(),
            status: action.response.status,
            body: action.response.body.clone(),
            headers,
            resource_type: None,
        });

        Ok(())
    }

    async fn captured_requests(&self) -> Vec<CapturedRequest> {
        let Some(network) = &self.network else {
            return Vec::new();
        };

        network.lock().await.captured_requests().to_vec()
    }
}

pub(crate) fn suite_requirements(suite: &TestSuite) -> RunnerRequirements {
    let mut requirements = RunnerRequirements::default();

    for step in suite
        .setup
        .iter()
        .chain(suite.steps.iter())
        .chain(suite.teardown.iter())
    {
        requirements.merge(step_requirements(step));
    }

    requirements
}

fn step_requirements(step: &TestStep) -> RunnerRequirements {
    let mut requirements = RunnerRequirements::default();

    if step.network_mock.is_some() {
        requirements.network_capture = true;
        requirements.network_interception = true;
    }

    if let Some(assertions) = &step.assert {
        for assertion in assertions {
            requirements.merge(assertion_requirements(assertion));
        }
    }

    requirements
}

pub(crate) fn assertion_requirements(assertion: &Assertion) -> RunnerRequirements {
    RunnerRequirements {
        network_capture: assertion.network_called.is_some(),
        network_interception: false,
    }
}

/// Execute a test suite against a browser instance.
pub async fn run_suite(
    suite: &TestSuite,
    config: &RunnerConfig,
    event_tx: Option<broadcast::Sender<TestEvent>>,
) -> Result<SuiteResult, TestError> {
    let suite_start = Instant::now();
    let started_at = chrono::Utc::now().to_rfc3339();

    let profiler = Profiler::new();

    // Configure viewport
    let viewport = suite
        .viewport
        .as_ref()
        .map(|v| ViewportConfig {
            width: v.width,
            height: v.height,
        })
        .unwrap_or_default();

    let browser = RayoBrowser::launch_with_config(profiler, viewport).await?;
    let page = browser.new_page().await?;

    let baseline_mgr = BaselineManager::new(config.baselines_dir.clone());
    let mut context = RunnerContext::new(suite_requirements(suite));

    let total_steps = suite.setup.len() + suite.steps.len() + suite.teardown.len();

    if let Some(tx) = &event_tx {
        let _ = tx.send(TestEvent::SuiteStarted {
            name: suite.name.clone(),
            total_steps,
        });
    }

    let base_url = config.base_url.as_deref();
    let step_results = match execute_suite_on_page(
        &page,
        suite,
        &baseline_mgr,
        &event_tx,
        base_url,
        config.abort_on_failure,
        &mut context,
    )
    .await
    {
        Ok(results) => results,
        Err(err) => {
            drop(page);
            browser.close().await;
            return Err(err);
        }
    };

    let passed = step_results.iter().filter(|r| r.pass).count();
    let failed = step_results.iter().filter(|r| !r.pass).count();

    let suite_result = SuiteResult {
        name: suite.name.clone(),
        pass: failed == 0,
        total_steps: step_results.len(),
        passed_steps: passed,
        failed_steps: failed,
        duration_ms: suite_start.elapsed().as_millis() as u64,
        steps: step_results,
        started_at,
    };

    if let Some(tx) = &event_tx {
        let _ = tx.send(TestEvent::SuiteCompleted {
            result: suite_result.clone(),
        });
    }

    // Clean up browser
    drop(page);
    browser.close().await;

    Ok(suite_result)
}

pub(crate) async fn execute_suite_on_page(
    page: &RayoPage,
    suite: &TestSuite,
    baseline_mgr: &BaselineManager,
    event_tx: &Option<broadcast::Sender<TestEvent>>,
    base_url: Option<&str>,
    abort_on_failure: bool,
    context: &mut RunnerContext,
) -> Result<Vec<StepResult>, TestError> {
    context.begin_scope(page).await?;

    let mut step_results = Vec::new();
    let mut step_index = 0;
    let mut had_failure = false;

    for step in &suite.setup {
        let result = run_step(
            page,
            step,
            baseline_mgr,
            event_tx,
            step_index,
            base_url,
            context,
        )
        .await;
        if !result.pass {
            had_failure = true;
        }
        step_results.push(result);
        step_index += 1;
        if had_failure && abort_on_failure {
            break;
        }
    }

    if !had_failure || !abort_on_failure {
        for step in &suite.steps {
            let result = run_step(
                page,
                step,
                baseline_mgr,
                event_tx,
                step_index,
                base_url,
                context,
            )
            .await;
            if !result.pass {
                had_failure = true;
            }
            step_results.push(result);
            step_index += 1;
            if had_failure && abort_on_failure {
                break;
            }
        }
    }

    for step in &suite.teardown {
        let result = run_step(
            page,
            step,
            baseline_mgr,
            event_tx,
            step_index,
            base_url,
            context,
        )
        .await;
        step_results.push(result);
        step_index += 1;
    }

    context.end_scope().await;

    Ok(step_results)
}

pub(crate) async fn run_step(
    page: &RayoPage,
    step: &TestStep,
    baseline_mgr: &BaselineManager,
    event_tx: &Option<broadcast::Sender<TestEvent>>,
    index: usize,
    base_url: Option<&str>,
    context: &mut RunnerContext,
) -> StepResult {
    let step_name = step
        .name
        .clone()
        .unwrap_or_else(|| format!("Step {}", index + 1));
    let step_start = Instant::now();

    if let Some(tx) = event_tx {
        let _ = tx.send(TestEvent::StepStarted {
            name: step_name.clone(),
            index,
        });
    }

    // Execute the action
    let action_name = step_action_name(step);
    let action_result = execute_action(page, step, base_url, context).await;

    let mut result = match action_result {
        Ok(()) => StepResult {
            name: step_name.clone(),
            pass: true,
            duration_ms: 0,
            action: action_name,
            error: None,
            assertions: Vec::new(),
            page_map: None,
        },
        Err(e) => StepResult {
            name: step_name.clone(),
            pass: false,
            duration_ms: 0,
            action: action_name,
            error: Some(e.to_string()),
            assertions: Vec::new(),
            page_map: None,
        },
    };

    // Run assertions if action succeeded
    if result.pass
        && let Some(assertions) = &step.assert
    {
        for assertion in assertions {
            let assertion_result = check_assertion(page, assertion, baseline_mgr, context).await;
            if !assertion_result.pass {
                result.pass = false;
                // Capture page map for debugging failed assertions
                if result.page_map.is_none()
                    && let Ok(pm) = page.page_map(None).await
                {
                    result.page_map = serde_json::to_value(&pm).ok();
                }
            }
            result.assertions.push(assertion_result);
        }
    }

    result.duration_ms = step_start.elapsed().as_millis() as u64;

    if let Some(tx) = event_tx {
        let _ = tx.send(TestEvent::StepCompleted {
            result: result.clone(),
            index,
        });
    }

    result
}

fn step_action_name(step: &TestStep) -> String {
    if step.navigate.is_some() {
        "navigate".into()
    } else if step.click.is_some() {
        "click".into()
    } else if step.r#type.is_some() {
        "type".into()
    } else if step.select.is_some() {
        "select".into()
    } else if step.scroll.is_some() {
        "scroll".into()
    } else if step.hover.is_some() {
        "hover".into()
    } else if step.press.is_some() {
        "press".into()
    } else if step.wait.is_some() {
        "wait".into()
    } else if step.batch.is_some() {
        "batch".into()
    } else if step.cookie.is_some() {
        "cookie".into()
    } else if step.network_mock.is_some() {
        "network_mock".into()
    } else {
        "assert_only".into()
    }
}

/// Resolve a URL against an optional base URL.
/// Relative paths (starting with `/`) get the base prepended.
/// Absolute URLs (containing `://`) are returned unchanged.
fn resolve_url(url: &str, base_url: Option<&str>) -> String {
    if url.contains("://") {
        return url.to_string();
    }
    match base_url {
        Some(base) => {
            let base = base.trim_end_matches('/');
            if url.starts_with('/') {
                format!("{base}{url}")
            } else {
                format!("{base}/{url}")
            }
        }
        None => url.to_string(),
    }
}

async fn execute_action(
    page: &RayoPage,
    step: &TestStep,
    base_url: Option<&str>,
    context: &mut RunnerContext,
) -> Result<(), TestError> {
    if let Some(url) = &step.navigate {
        let resolved = resolve_url(url, base_url);
        page.goto(&resolved).await?;
    } else if let Some(target) = &step.click {
        let sel = target.to_selector();
        page.click(sel.as_deref(), None).await?;
    } else if let Some(action) = &step.r#type {
        page.type_text(Some(&action.selector), None, &action.value, true)
            .await?;
    } else if let Some(action) = &step.select {
        page.select_option(Some(&action.selector), None, &action.value)
            .await?;
    } else if let Some(target) = &step.hover {
        let sel = target.to_selector();
        page.hover(sel.as_deref(), None).await?;
    } else if let Some(key) = &step.press {
        page.press_key(None, None, key).await?;
    } else if let Some(action) = &step.wait {
        if let Some(sel) = &action.selector {
            page.wait_for_selector(sel, action.timeout_ms, true).await?;
        }
        if let Some(text) = &action.text {
            wait_for_page_text(page, text, action.timeout_ms).await?;
        }
        if let Some(element_text) = &action.element_text {
            wait_for_element_text(
                page,
                &element_text.selector,
                &element_text.contains,
                action.timeout_ms,
            )
            .await?;
        }
        if action.network_idle.unwrap_or(false) {
            page.wait_for_network_idle(500, action.timeout_ms).await?;
        }
    } else if let Some(actions) = &step.batch {
        let batch_actions: Vec<BatchAction> = actions
            .iter()
            .filter_map(|a| to_batch_action(a, base_url))
            .collect();
        page.execute_batch(batch_actions, false).await?;
    } else if let Some(action) = &step.network_mock {
        context.add_mock_rule(page, action).await?;
    }
    // assert_only steps have no action — that's valid
    Ok(())
}

async fn wait_for_page_text(
    page: &RayoPage,
    expected: &str,
    timeout_ms: u64,
) -> Result<(), TestError> {
    wait_for_text_condition(page, None, expected, timeout_ms).await
}

async fn wait_for_element_text(
    page: &RayoPage,
    selector: &str,
    expected: &str,
    timeout_ms: u64,
) -> Result<(), TestError> {
    wait_for_text_condition(page, Some(selector), expected, timeout_ms).await
}

async fn wait_for_text_condition(
    page: &RayoPage,
    selector: Option<&str>,
    expected: &str,
    timeout_ms: u64,
) -> Result<(), TestError> {
    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);

    loop {
        let text = page.text_content(selector, 200).await?;
        if text_contains_case_insensitive(&text, expected) {
            return Ok(());
        }

        if start.elapsed() >= timeout {
            return Err(TestError::Other(format_wait_timeout(
                selector, expected, timeout_ms, &text,
            )));
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn text_contains_case_insensitive(haystack: &str, needle: &str) -> bool {
    haystack
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

fn format_wait_timeout(
    selector: Option<&str>,
    expected: &str,
    timeout_ms: u64,
    observed: &str,
) -> String {
    let observed = truncate_text_for_message(observed);
    match selector {
        Some(selector) => format!(
            "Timed out after {timeout_ms}ms waiting for selector '{selector}' to contain text '{expected}'. Last observed text: {observed}"
        ),
        None => format!(
            "Timed out after {timeout_ms}ms waiting for text '{expected}' to appear. Last page text: {observed}"
        ),
    }
}

fn truncate_text_for_message(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return "(empty)".into();
    }

    const MAX_LEN: usize = 160;
    if normalized.len() <= MAX_LEN {
        normalized
    } else {
        format!("{}...", &normalized[..MAX_LEN])
    }
}

fn to_batch_action(a: &BatchStepAction, base_url: Option<&str>) -> Option<BatchAction> {
    let target = if let Some(sel) = &a.selector {
        ActionTarget::Selector {
            selector: sel.clone(),
        }
    } else if let Some(id) = a.id {
        ActionTarget::Id { id }
    } else if a.action == "goto" {
        // goto doesn't need a target
        ActionTarget::Selector {
            selector: String::new(),
        }
    } else {
        return None;
    };

    match a.action.as_str() {
        "click" => Some(BatchAction::Click { target }),
        "type" => Some(BatchAction::Type {
            target,
            value: a.value.clone().unwrap_or_default(),
        }),
        "select" => Some(BatchAction::Select {
            target,
            value: a.value.clone().unwrap_or_default(),
        }),
        "press" => Some(BatchAction::Press {
            target: Some(target),
            key: a.key.clone().unwrap_or_default(),
        }),
        "hover" => Some(BatchAction::Hover { target }),
        "goto" => Some(BatchAction::Goto {
            url: resolve_url(&a.url.clone().unwrap_or_default(), base_url),
        }),
        _ => None,
    }
}

pub(crate) async fn check_assertion(
    page: &RayoPage,
    assertion: &Assertion,
    baseline_mgr: &BaselineManager,
    context: &RunnerContext,
) -> AssertionResult {
    if let Some(criteria) = &assertion.page_map_contains {
        return check_page_map(page, criteria).await;
    }
    if let Some(expected) = &assertion.text_contains {
        return check_text_contains(page, expected).await;
    }
    if let Some(config) = &assertion.screenshot {
        return check_screenshot(page, config, baseline_mgr).await;
    }
    if let Some(expected) = &assertion.network_called {
        return check_network_called(expected, context).await;
    }
    AssertionResult {
        assertion_type: "unknown".into(),
        pass: false,
        message: Some("No assertion type specified".into()),
        diff_report: None,
        new_baseline: false,
    }
}

async fn check_network_called(
    expected: &NetworkAssertion,
    context: &RunnerContext,
) -> AssertionResult {
    let requests = context.captured_requests().await;
    let matches: Vec<&CapturedRequest> = requests
        .iter()
        .filter(|request| network_request_matches(request, expected))
        .collect();

    let pass = !matches.is_empty();
    let method_hint = expected
        .method
        .as_deref()
        .map(|method| format!(" and method '{method}'"))
        .unwrap_or_default();

    AssertionResult {
        assertion_type: "network_called".into(),
        pass,
        message: if pass {
            None
        } else if requests.is_empty() {
            Some("No network requests were captured in this suite scope".into())
        } else {
            Some(format_network_assertion_failure(
                expected,
                &requests,
                &method_hint,
            ))
        },
        diff_report: None,
        new_baseline: false,
    }
}

fn format_network_assertion_failure(
    expected: &NetworkAssertion,
    requests: &[CapturedRequest],
    method_hint: &str,
) -> String {
    let preview = requests
        .iter()
        .take(5)
        .map(|request| format!("{} {}", request.method, request.url))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "No captured request matched url '{}'{} ({} requests captured). Captured requests: {}",
        expected.url,
        method_hint,
        requests.len(),
        preview
    )
}

fn network_request_matches(request: &CapturedRequest, expected: &NetworkAssertion) -> bool {
    let method_matches = expected
        .method
        .as_deref()
        .is_none_or(|method| request.method.eq_ignore_ascii_case(method));

    method_matches && url_pattern_matches(&request.url, &expected.url)
}

fn url_pattern_matches(url: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if pattern.contains('*') {
        let parts: Vec<&str> = pattern.split('*').collect();
        let mut pos = 0;

        for (index, part) in parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }

            match url[pos..].find(part) {
                Some(found) => {
                    if index == 0 && found != 0 {
                        return false;
                    }
                    pos += found + part.len();
                }
                None => return false,
            }
        }

        if !pattern.ends_with('*')
            && let Some(last) = parts.last()
            && !last.is_empty()
        {
            return url.ends_with(last);
        }

        true
    } else {
        url.contains(pattern)
    }
}

async fn check_page_map(page: &RayoPage, criteria: &PageMapAssertion) -> AssertionResult {
    let page_map = match page.page_map(None).await {
        Ok(pm) => pm,
        Err(e) => {
            return AssertionResult {
                assertion_type: "page_map_contains".into(),
                pass: false,
                message: Some(format!("Failed to get page map: {e}")),
                diff_report: None,
                new_baseline: false,
            };
        }
    };

    let found = page_map.interactive.iter().any(|el| {
        let mut matches = true;
        if let Some(sel) = &criteria.selector {
            matches = matches && el.selector.contains(sel.as_str());
        }
        if let Some(text) = &criteria.text {
            let el_text = el.text.as_deref().unwrap_or("");
            let el_label = el.label.as_deref().unwrap_or("");
            matches =
                matches && (el_text.contains(text.as_str()) || el_label.contains(text.as_str()));
        }
        if let Some(role) = &criteria.role {
            matches = matches && el.role.as_deref() == Some(role);
        }
        if let Some(tag) = &criteria.tag {
            matches = matches && el.tag == *tag;
        }
        matches
    });

    AssertionResult {
        assertion_type: "page_map_contains".into(),
        pass: found,
        message: if found {
            None
        } else {
            Some(format!("No element matching criteria: {criteria:?}"))
        },
        diff_report: None,
        new_baseline: false,
    }
}

async fn check_text_contains(page: &RayoPage, expected: &str) -> AssertionResult {
    let text = match page.text_content(None, 50).await {
        Ok(t) => t,
        Err(e) => {
            return AssertionResult {
                assertion_type: "text_contains".into(),
                pass: false,
                message: Some(format!("Failed to get text: {e}")),
                diff_report: None,
                new_baseline: false,
            };
        }
    };

    let found = text.contains(expected);
    AssertionResult {
        assertion_type: "text_contains".into(),
        pass: found,
        message: if found {
            None
        } else {
            Some(format!("Text '{expected}' not found on page"))
        },
        diff_report: None,
        new_baseline: false,
    }
}

async fn check_screenshot(
    page: &RayoPage,
    config: &ScreenshotAssertion,
    baseline_mgr: &BaselineManager,
) -> AssertionResult {
    // Freeze animations for stable screenshots
    let _ = page.freeze_animations().await;

    // Capture current screenshot as PNG
    let current_png = match &config.selector {
        Some(sel) => page.screenshot_element(sel).await,
        None => page.screenshot_png(config.full_page).await,
    };

    let _ = page.unfreeze_animations().await;

    let current_png = match current_png {
        Ok(bytes) => bytes,
        Err(e) => {
            return AssertionResult {
                assertion_type: "screenshot".into(),
                pass: false,
                message: Some(format!("Screenshot capture failed: {e}")),
                diff_report: None,
                new_baseline: false,
            };
        }
    };

    // Check if baseline exists
    if !baseline_mgr.exists(&config.name) {
        // Auto-create baseline
        let img = match image::load_from_memory(&current_png) {
            Ok(img) => img,
            Err(e) => {
                return AssertionResult {
                    assertion_type: "screenshot".into(),
                    pass: false,
                    message: Some(format!("Failed to decode screenshot: {e}")),
                    diff_report: None,
                    new_baseline: false,
                };
            }
        };
        let (w, h) = image::GenericImageView::dimensions(&img);
        if let Err(e) = baseline_mgr.save(&config.name, &current_png, w, h) {
            return AssertionResult {
                assertion_type: "screenshot".into(),
                pass: false,
                message: Some(format!("Failed to save baseline: {e}")),
                diff_report: None,
                new_baseline: false,
            };
        }
        return AssertionResult {
            assertion_type: "screenshot".into(),
            pass: true,
            message: Some(format!("New baseline created: {}", config.name)),
            diff_report: None,
            new_baseline: true,
        };
    }

    // Load baseline and compare
    let baseline_png = match baseline_mgr.load(&config.name) {
        Ok(b) => b,
        Err(e) => {
            return AssertionResult {
                assertion_type: "screenshot".into(),
                pass: false,
                message: Some(format!("Failed to load baseline: {e}")),
                diff_report: None,
                new_baseline: false,
            };
        }
    };

    let diff_opts = rayo_visual::DiffOptions {
        threshold: config.threshold,
        ..Default::default()
    };

    match rayo_visual::compare(&baseline_png, &current_png, &diff_opts) {
        Ok(report) => {
            let diff_json = serde_json::to_value(&report).ok();
            AssertionResult {
                assertion_type: "screenshot".into(),
                pass: report.pass,
                message: if report.pass {
                    None
                } else {
                    Some(format!(
                        "Visual diff: {:.1}% pixels changed (threshold: {:.1}%), perceptual score: {:.3}",
                        report.diff_ratio * 100.0,
                        config.threshold * 100.0,
                        report.perceptual_score,
                    ))
                },
                diff_report: diff_json,
                new_baseline: false,
            }
        }
        Err(e) => AssertionResult {
            assertion_type: "screenshot".into(),
            pass: false,
            message: Some(format!("Diff failed: {e}")),
            diff_report: None,
            new_baseline: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rayo_core::RayoBrowser;

    async fn launch_test_page() -> Option<(RayoBrowser, RayoPage)> {
        let browser = match RayoBrowser::launch().await {
            Ok(browser) => browser,
            Err(_) => {
                eprintln!("SKIP: Chrome not available");
                return None;
            }
        };

        let page = browser.new_page().await.expect("Failed to create page");
        page.goto("about:blank")
            .await
            .expect("Failed to load blank page");
        Some((browser, page))
    }

    #[test]
    fn resolve_url_absolute_unchanged() {
        assert_eq!(
            resolve_url("https://example.com/page", Some("http://localhost:3000")),
            "https://example.com/page"
        );
        assert_eq!(
            resolve_url("http://other.dev/api", Some("http://localhost:3000")),
            "http://other.dev/api"
        );
    }

    #[test]
    fn resolve_url_relative_with_base() {
        assert_eq!(
            resolve_url("/", Some("http://localhost:3000")),
            "http://localhost:3000/"
        );
        assert_eq!(
            resolve_url("/blog", Some("http://localhost:3000")),
            "http://localhost:3000/blog"
        );
        assert_eq!(
            resolve_url("/api/v1/users", Some("http://localhost:3000")),
            "http://localhost:3000/api/v1/users"
        );
    }

    #[test]
    fn resolve_url_trailing_slash_on_base() {
        assert_eq!(
            resolve_url("/page", Some("http://localhost:3000/")),
            "http://localhost:3000/page"
        );
    }

    #[test]
    fn resolve_url_no_leading_slash() {
        assert_eq!(
            resolve_url("page", Some("http://localhost:3000")),
            "http://localhost:3000/page"
        );
    }

    #[test]
    fn resolve_url_no_base() {
        assert_eq!(resolve_url("/page", None), "/page");
        assert_eq!(
            resolve_url("https://example.com", None),
            "https://example.com"
        );
    }

    #[test]
    fn text_wait_matching_is_case_insensitive() {
        assert!(text_contains_case_insensitive(
            "Agent Is Building",
            "is building"
        ));
        assert!(!text_contains_case_insensitive("Agent is idle", "building"));
    }

    #[test]
    fn format_wait_timeout_includes_last_observed_text() {
        let message = format_wait_timeout(
            None,
            "is building",
            1500,
            "Agent is planning the work right now",
        );
        assert!(message.contains("Timed out after 1500ms"));
        assert!(message.contains("is building"));
        assert!(message.contains("Agent is planning the work right now"));
    }

    #[test]
    fn network_request_matches_method_and_url_pattern() {
        let request = CapturedRequest {
            url: "https://example.com/api/users?page=1".into(),
            method: "POST".into(),
            resource_type: "Fetch".into(),
            status: Some(200),
            headers: Vec::new(),
            timestamp_ms: 0.0,
            request_id: None,
        };
        let assertion = NetworkAssertion {
            url: "*/api/users*".into(),
            method: Some("post".into()),
        };

        assert!(network_request_matches(&request, &assertion));
    }

    #[test]
    fn network_request_does_not_match_wrong_method() {
        let request = CapturedRequest {
            url: "https://example.com/api/users".into(),
            method: "GET".into(),
            resource_type: "Fetch".into(),
            status: Some(200),
            headers: Vec::new(),
            timestamp_ms: 0.0,
            request_id: None,
        };
        let assertion = NetworkAssertion {
            url: "/api/users".into(),
            method: Some("POST".into()),
        };

        assert!(!network_request_matches(&request, &assertion));
    }

    #[tokio::test]
    async fn wait_for_page_text_matches_case_insensitively() {
        let Some((browser, page)) = launch_test_page().await else {
            return;
        };

        page.evaluate(
            "setTimeout(() => { document.body.innerHTML = '<div>Agent Is Building</div>'; }, 100);",
        )
        .await
        .expect("Failed to schedule page update");

        wait_for_page_text(&page, "is building", 3000)
            .await
            .expect("wait.text should resolve");

        browser.close().await;
    }

    #[tokio::test]
    async fn wait_for_element_text_timeout_reports_last_observed_text() {
        let Some((browser, page)) = launch_test_page().await else {
            return;
        };

        page.evaluate("document.body.innerHTML = '<div id=\"status\">Waiting for response</div>';")
            .await
            .expect("Failed to seed page");

        let err = wait_for_element_text(&page, "#status", "ready", 250)
            .await
            .expect_err("wait.element_text should time out");

        let message = err.to_string();
        assert!(message.contains("Timed out after 250ms"));
        assert!(message.contains("#status"));
        assert!(message.contains("Waiting for response"));

        browser.close().await;
    }
}
