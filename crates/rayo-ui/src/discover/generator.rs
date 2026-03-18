//! YAML test generation from detected flows.
//!
//! Converts `DetectedFlow` instances into `TestSuite` YAML files,
//! using the existing types from `crate::types`.

use crate::error::TestError;
use crate::types::*;

use super::flows::{DetectedFlow, FlowStep, FlowType};
use std::collections::HashMap;
use std::path::Path;

/// Exploration result for a single page.
#[derive(Debug, Clone)]
pub struct ExploredPage {
    pub url: String,
    pub status: PageStatus,
    pub console_errors: usize,
    pub has_elements: bool,
}

/// Status of a page after exploration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageStatus {
    Ok,
    Error,
    Redirect,
    AuthGated,
}

impl PageStatus {
    pub fn as_str(&self) -> &str {
        match self {
            PageStatus::Ok => "ok",
            PageStatus::Error => "error",
            PageStatus::Redirect => "redirect",
            PageStatus::AuthGated => "auth-gated",
        }
    }
}

/// Generate test suites from detected flows and explored pages.
pub fn generate_test_suites(
    flows: &[DetectedFlow],
    explored_pages: &[ExploredPage],
    base_url: &str,
) -> Vec<(String, TestSuite)> {
    let mut suites = Vec::new();

    // Generate a test suite for each flow
    for flow in flows {
        let filename = flow_to_filename(flow);
        let suite = flow_to_suite(flow);
        suites.push((filename, suite));
    }

    // Generate smoke test that visits every discovered page
    let smoke_suite = generate_smoke_suite(explored_pages, base_url);
    suites.push(("_smoke.test.yaml".into(), smoke_suite));

    suites
}

/// Write generated test suites to disk.
pub fn write_test_suites(
    suites: &[(String, TestSuite)],
    tests_dir: &Path,
    force: bool,
) -> Result<usize, TestError> {
    std::fs::create_dir_all(tests_dir)?;

    let mut written = 0;
    for (filename, suite) in suites {
        let path = tests_dir.join(filename);

        if path.exists() && !force {
            tracing::info!("Skipping existing test file: {}", path.display());
            continue;
        }

        let yaml = serde_yaml::to_string(suite)
            .map_err(|e| TestError::Other(format!("Failed to serialize test suite: {e}")))?;

        std::fs::write(&path, yaml)?;
        tracing::info!("Wrote test file: {}", path.display());
        written += 1;
    }

    Ok(written)
}

/// Convert a flow to a test suite filename.
fn flow_to_filename(flow: &DetectedFlow) -> String {
    let slug = super::flows::slug_from_url(&flow.url);
    format!("{}-{}.test.yaml", slug, flow.flow_type.as_str())
}

/// Convert a detected flow into a TestSuite.
fn flow_to_suite(flow: &DetectedFlow) -> TestSuite {
    let steps: Vec<TestStep> = flow.steps.iter().map(flow_step_to_test_step).collect();

    TestSuite {
        name: flow.name.clone(),
        viewport: None,
        setup: Vec::new(),
        steps,
        teardown: Vec::new(),
    }
}

/// Convert a FlowStep into a TestStep.
fn flow_step_to_test_step(step: &FlowStep) -> TestStep {
    let mut test_step = TestStep {
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
    };

    match step.action.as_str() {
        "navigate" => {
            test_step.name = Some("Navigate to page".into());
            test_step.navigate = step.url.clone();
        }
        "type" => {
            if let (Some(selector), Some(value)) = (&step.selector, &step.value) {
                test_step.name = Some(format!("Type into {selector}"));
                test_step.r#type = Some(TypeAction {
                    selector: selector.clone(),
                    value: value.clone(),
                });
            }
        }
        "click" => {
            if let Some(selector) = &step.selector {
                test_step.name = Some(format!("Click {selector}"));
                test_step.click = Some(SelectorTarget::Selector(selector.clone()));
            }
        }
        "select" => {
            if let (Some(selector), Some(value)) = (&step.selector, &step.value) {
                test_step.name = Some(format!("Select {value} in {selector}"));
                test_step.select = Some(SelectAction {
                    selector: selector.clone(),
                    value: value.clone(),
                });
            }
        }
        "press" => {
            let key = step.value.clone().unwrap_or_else(|| "Enter".into());
            test_step.name = Some(format!("Press {key}"));
            test_step.press = Some(key);
        }
        "assert" => {
            test_step.name = Some("Verify page loaded".into());
            test_step.assert = Some(vec![Assertion {
                page_map_contains: Some(PageMapAssertion {
                    selector: None,
                    text: None,
                    role: None,
                    tag: None,
                }),
                text_contains: None,
                screenshot: None,
                network_called: None,
            }]);
        }
        _ => {}
    }

    test_step
}

/// Generate a smoke test that visits every page and asserts basic loading.
fn generate_smoke_suite(pages: &[ExploredPage], base_url: &str) -> TestSuite {
    let mut steps = Vec::new();

    // Group pages by status for a cleaner test
    let ok_pages: Vec<&ExploredPage> = pages
        .iter()
        .filter(|p| p.status == PageStatus::Ok)
        .collect();

    for page in &ok_pages {
        // Navigate step
        let url = if page.url.starts_with("http") {
            page.url.clone()
        } else {
            format!("{}{}", base_url.trim_end_matches('/'), page.url)
        };

        steps.push(TestStep {
            name: Some(format!("Load {}", page.url)),
            navigate: Some(url),
            click: None,
            r#type: None,
            select: None,
            scroll: None,
            hover: None,
            press: None,
            wait: None,
            batch: None,
            cookie: None,
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
        });
    }

    // If no pages loaded successfully, add at least one step for the base URL
    if steps.is_empty() {
        steps.push(TestStep {
            name: Some("Load base URL".into()),
            navigate: Some(base_url.to_string()),
            click: None,
            r#type: None,
            select: None,
            scroll: None,
            hover: None,
            press: None,
            wait: None,
            batch: None,
            cookie: None,
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
        });
    }

    TestSuite {
        name: "Smoke Test — All Pages".into(),
        viewport: None,
        setup: Vec::new(),
        steps,
        teardown: Vec::new(),
    }
}

/// Group flows by page slug for deduplication.
pub fn deduplicate_flows(flows: Vec<DetectedFlow>) -> Vec<DetectedFlow> {
    let mut seen: HashMap<String, Vec<FlowType>> = HashMap::new();
    let mut result = Vec::new();

    for flow in flows {
        let key = super::flows::slug_from_url(&flow.url);
        let entry = seen.entry(key).or_default();

        if !entry.contains(&flow.flow_type) {
            entry.push(flow.flow_type.clone());
            result.push(flow);
        }
    }

    result
}
