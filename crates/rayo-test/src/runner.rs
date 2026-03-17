//! Test runner — executes test suites against a browser.

use crate::error::TestError;
use crate::result::{AssertionResult, StepResult, SuiteResult};
use crate::types::*;
use rayo_core::batch::{ActionTarget, BatchAction};
use rayo_core::{RayoBrowser, RayoPage, ViewportConfig};
use rayo_profiler::Profiler;
use rayo_visual::BaselineManager;
use std::path::PathBuf;
use std::time::Instant;
use tokio::sync::broadcast;

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
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            baselines_dir: PathBuf::from(".rayo/baselines"),
            abort_on_failure: false,
        }
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

    let total_steps = suite.setup.len() + suite.steps.len() + suite.teardown.len();

    if let Some(tx) = &event_tx {
        let _ = tx.send(TestEvent::SuiteStarted {
            name: suite.name.clone(),
            total_steps,
        });
    }

    let mut step_results = Vec::new();
    let mut step_index = 0;
    let mut had_failure = false;

    // Run setup steps
    for step in &suite.setup {
        let result = run_step(&page, step, &baseline_mgr, &event_tx, step_index).await;
        if !result.pass {
            had_failure = true;
        }
        step_results.push(result);
        step_index += 1;
        if had_failure && config.abort_on_failure {
            break;
        }
    }

    // Run test steps
    if !had_failure || !config.abort_on_failure {
        for step in &suite.steps {
            let result = run_step(&page, step, &baseline_mgr, &event_tx, step_index).await;
            if !result.pass {
                had_failure = true;
            }
            step_results.push(result);
            step_index += 1;
            if had_failure && config.abort_on_failure {
                break;
            }
        }
    }

    // Run teardown steps (always, even on failure)
    for step in &suite.teardown {
        let result = run_step(&page, step, &baseline_mgr, &event_tx, step_index).await;
        step_results.push(result);
        step_index += 1;
    }

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

async fn run_step(
    page: &RayoPage,
    step: &TestStep,
    baseline_mgr: &BaselineManager,
    event_tx: &Option<broadcast::Sender<TestEvent>>,
    index: usize,
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
    let action_result = execute_action(page, step).await;

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
            let assertion_result = check_assertion(page, assertion, baseline_mgr).await;
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
    } else {
        "assert_only".into()
    }
}

async fn execute_action(page: &RayoPage, step: &TestStep) -> Result<(), TestError> {
    if let Some(url) = &step.navigate {
        page.goto(url).await?;
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
        } else if action.network_idle.unwrap_or(false) {
            page.wait_for_network_idle(500, action.timeout_ms).await?;
        }
    } else if let Some(actions) = &step.batch {
        let batch_actions: Vec<BatchAction> = actions.iter().filter_map(to_batch_action).collect();
        page.execute_batch(batch_actions, false).await?;
    }
    // assert_only steps have no action — that's valid
    Ok(())
}

fn to_batch_action(a: &BatchStepAction) -> Option<BatchAction> {
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
            url: a.url.clone().unwrap_or_default(),
        }),
        _ => None,
    }
}

async fn check_assertion(
    page: &RayoPage,
    assertion: &Assertion,
    baseline_mgr: &BaselineManager,
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
    if assertion.network_called.is_some() {
        return AssertionResult {
            assertion_type: "network_called".into(),
            pass: true,
            message: Some("network assertions not yet implemented".into()),
            diff_report: None,
            new_baseline: false,
        };
    }
    AssertionResult {
        assertion_type: "unknown".into(),
        pass: false,
        message: Some("No assertion type specified".into()),
        diff_report: None,
        new_baseline: false,
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
