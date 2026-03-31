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
        network_mock: None,
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
            network_mock: None,
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
            network_mock: None,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_flow(name: &str, url: &str, flow_type: FlowType) -> DetectedFlow {
        DetectedFlow {
            name: name.into(),
            flow_type,
            steps: vec![
                FlowStep {
                    action: "navigate".into(),
                    selector: None,
                    value: None,
                    url: Some(url.into()),
                    assertion: None,
                },
                FlowStep {
                    action: "type".into(),
                    selector: Some("input[name=email]".into()),
                    value: Some("test@example.com".into()),
                    url: None,
                    assertion: None,
                },
                FlowStep {
                    action: "click".into(),
                    selector: Some("button[type=submit]".into()),
                    value: None,
                    url: None,
                    assertion: None,
                },
                FlowStep {
                    action: "assert".into(),
                    selector: None,
                    value: None,
                    url: None,
                    assertion: Some("page_map_contains_any".into()),
                },
            ],
            url: url.into(),
        }
    }

    #[test]
    fn test_generate_yaml_from_flow_is_valid() {
        let flow = make_flow("Login Flow", "http://localhost:3000/login", FlowType::Auth);

        let suites = generate_test_suites(&[flow], &[], "http://localhost:3000");

        // Should have at least 2 suites: the flow suite + smoke test
        assert!(suites.len() >= 2, "Should generate flow suite + smoke test");

        // The flow suite should be valid YAML
        let (filename, suite) = &suites[0];
        assert!(
            filename.contains("auth"),
            "Auth flow filename should contain 'auth'"
        );

        let yaml = serde_yaml::to_string(suite).expect("Should serialize to valid YAML");
        assert!(!yaml.is_empty());

        // Verify it can be deserialized back
        let _parsed: TestSuite =
            serde_yaml::from_str(&yaml).expect("Should deserialize back from YAML");
    }

    #[test]
    fn test_generate_smoke_test_has_one_step_per_page() {
        let pages = vec![
            ExploredPage {
                url: "/".into(),
                status: PageStatus::Ok,
                console_errors: 0,
                has_elements: true,
            },
            ExploredPage {
                url: "/about".into(),
                status: PageStatus::Ok,
                console_errors: 0,
                has_elements: true,
            },
            ExploredPage {
                url: "/error-page".into(),
                status: PageStatus::Error,
                console_errors: 3,
                has_elements: false,
            },
        ];

        let suites = generate_test_suites(&[], &pages, "http://localhost:3000");

        // Should have the smoke test
        let smoke = suites
            .iter()
            .find(|(name, _)| name.contains("smoke"))
            .expect("Should have a smoke test suite");

        let (_, smoke_suite) = smoke;
        assert_eq!(smoke_suite.name, "Smoke Test \u{2014} All Pages");

        // Smoke test should only include OK pages (not error pages)
        assert_eq!(
            smoke_suite.steps.len(),
            2,
            "Smoke test should have one step per OK page"
        );

        // Each step should have a navigate URL and an assertion
        for step in &smoke_suite.steps {
            assert!(step.navigate.is_some(), "Each smoke step should navigate");
            assert!(step.assert.is_some(), "Each smoke step should assert");
        }
    }

    #[test]
    fn test_flow_to_suite_has_correct_steps() {
        let flow = make_flow(
            "Contact Form",
            "http://localhost:3000/contact",
            FlowType::Form,
        );

        let suite = flow_to_suite(&flow);
        assert_eq!(suite.name, "Contact Form");
        assert_eq!(suite.steps.len(), 4); // navigate, type, click, assert

        // First step should be navigate
        assert!(suite.steps[0].navigate.is_some());
        // Second step should be type
        assert!(suite.steps[1].r#type.is_some());
        // Third step should be click
        assert!(suite.steps[2].click.is_some());
        // Fourth step should be assert
        assert!(suite.steps[3].assert.is_some());
    }

    #[test]
    fn test_flow_to_filename() {
        let flow = make_flow("Login Flow", "http://localhost:3000/login", FlowType::Auth);
        let filename = flow_to_filename(&flow);
        assert_eq!(filename, "login-auth.test.yaml");

        let flow = make_flow("Home Form", "http://localhost:3000/", FlowType::Form);
        let filename = flow_to_filename(&flow);
        assert_eq!(filename, "home-form.test.yaml");
    }

    #[test]
    fn test_deduplicate_flows() {
        let flows = vec![
            make_flow("Login Auth", "http://localhost:3000/login", FlowType::Auth),
            make_flow(
                "Login Auth Duplicate",
                "http://localhost:3000/login",
                FlowType::Auth,
            ),
            make_flow("Login Form", "http://localhost:3000/login", FlowType::Form),
        ];

        let deduped = deduplicate_flows(flows);
        assert_eq!(
            deduped.len(),
            2,
            "Should keep Auth + Form, drop duplicate Auth"
        );

        let types: Vec<&FlowType> = deduped.iter().map(|f| &f.flow_type).collect();
        assert!(types.contains(&&FlowType::Auth));
        assert!(types.contains(&&FlowType::Form));
    }

    #[test]
    fn test_write_test_suites() {
        let dir = std::env::temp_dir().join("rayo_test_generator_write");
        let _ = std::fs::remove_dir_all(&dir);

        let flow = make_flow("Test Flow", "http://localhost:3000/test", FlowType::Form);
        let suites = generate_test_suites(&[flow], &[], "http://localhost:3000");

        let written = write_test_suites(&suites, &dir, false).expect("Should write suites");
        assert!(written > 0, "Should write at least one file");

        // Verify files exist on disk
        for (filename, _) in &suites {
            let path = dir.join(filename);
            assert!(path.exists(), "File should exist: {}", path.display());
        }

        // Writing again without force should skip existing files
        let written_again =
            write_test_suites(&suites, &dir, false).expect("Should succeed even if files exist");
        assert_eq!(written_again, 0, "Should skip existing files without force");

        // Writing with force should overwrite
        let written_force =
            write_test_suites(&suites, &dir, true).expect("Should overwrite with force");
        assert!(written_force > 0, "Should overwrite with force=true");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
