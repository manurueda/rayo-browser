//! Tests for rayo-ui types — serde roundtrips, defaults, constructors.

use rayo_ui::types::*;

// ---------------------------------------------------------------------------
// TestSuite
// ---------------------------------------------------------------------------

#[test]
fn test_suite_minimal_yaml_roundtrip() {
    let yaml = "name: Minimal\nsteps:\n  - navigate: \"http://example.com\"\n";
    let suite: TestSuite = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(suite.name, "Minimal");
    assert_eq!(suite.steps.len(), 1);
    assert!(suite.viewport.is_none());
    assert!(suite.setup.is_empty());
    assert!(suite.teardown.is_empty());
    let serialized = serde_yaml::to_string(&suite).unwrap();
    let reparsed: TestSuite = serde_yaml::from_str(&serialized).unwrap();
    assert_eq!(reparsed.name, suite.name);
    assert_eq!(reparsed.steps.len(), suite.steps.len());
}

#[test]
fn test_suite_full_yaml_roundtrip() {
    let yaml = concat!(
        "name: Full Suite\n",
        "viewport:\n  width: 1920\n  height: 1080\n",
        "setup:\n  - navigate: \"http://example.com/setup\"\n",
        "steps:\n",
        "  - name: \"Navigate home\"\n    navigate: \"http://example.com\"\n",
        "  - name: \"Click button\"\n    click: \"#submit\"\n",
        "teardown:\n  - navigate: \"http://example.com/logout\"\n",
    );
    let suite: TestSuite = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(suite.name, "Full Suite");
    assert!(suite.viewport.is_some());
    let vp = suite.viewport.as_ref().unwrap();
    assert_eq!(vp.width, 1920);
    assert_eq!(vp.height, 1080);
    assert_eq!(suite.setup.len(), 1);
    assert_eq!(suite.steps.len(), 2);
    assert_eq!(suite.teardown.len(), 1);
}

// ---------------------------------------------------------------------------
// ViewportDef
// ---------------------------------------------------------------------------

#[test]
fn test_viewport_defaults() {
    let yaml = "{}";
    let vp: ViewportDef = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(vp.width, 1280);
    assert_eq!(vp.height, 720);
}

#[test]
fn test_viewport_custom() {
    let yaml = "width: 800\nheight: 600";
    let vp: ViewportDef = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(vp.width, 800);
    assert_eq!(vp.height, 600);
}

#[test]
fn test_viewport_partial_defaults() {
    let yaml = "width: 1024";
    let vp: ViewportDef = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(vp.width, 1024);
    assert_eq!(vp.height, 720);
}

// ---------------------------------------------------------------------------
// TestStep
// ---------------------------------------------------------------------------

#[test]
fn test_step_navigate() {
    let yaml = "navigate: 'http://example.com'";
    let step: TestStep = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(step.navigate, Some("http://example.com".into()));
    assert!(step.click.is_none());
    assert!(step.r#type.is_none());
}

#[test]
fn test_step_click_string_selector() {
    let yaml = "click: '#my-button'";
    let step: TestStep = serde_yaml::from_str(yaml).unwrap();
    assert!(step.click.is_some());
    let target = step.click.unwrap();
    assert_eq!(target.to_selector(), Some("#my-button".into()));
}

#[test]
fn test_step_click_structured() {
    let yaml = "click:\n  selector: \".btn\"\n  text: \"Submit\"\n";
    let step: TestStep = serde_yaml::from_str(yaml).unwrap();
    let target = step.click.unwrap();
    assert_eq!(target.to_selector(), Some(".btn".into()));
    match target {
        SelectorTarget::Structured { text, .. } => {
            assert_eq!(text, Some("Submit".into()));
        }
        _ => panic!("Expected Structured variant"),
    }
}

#[test]
fn test_step_type_action() {
    let yaml = "type:\n  selector: \"input[name=email]\"\n  value: \"test@example.com\"\n";
    let step: TestStep = serde_yaml::from_str(yaml).unwrap();
    let type_action = step.r#type.unwrap();
    assert_eq!(type_action.selector, "input[name=email]");
    assert_eq!(type_action.value, "test@example.com");
}

#[test]
fn test_step_select_action() {
    let yaml = "select:\n  selector: \"#country\"\n  value: \"US\"\n";
    let step: TestStep = serde_yaml::from_str(yaml).unwrap();
    let select_action = step.select.unwrap();
    assert_eq!(select_action.selector, "#country");
    assert_eq!(select_action.value, "US");
}

#[test]
fn test_step_scroll_action() {
    let yaml = "scroll:\n  selector: \"#content\"\n  x: 0\n  y: 500\n";
    let step: TestStep = serde_yaml::from_str(yaml).unwrap();
    let scroll = step.scroll.unwrap();
    assert_eq!(scroll.selector, Some("#content".into()));
    assert_eq!(scroll.x, Some(0));
    assert_eq!(scroll.y, Some(500));
}

#[test]
fn test_step_scroll_defaults() {
    let yaml = "scroll: {}";
    let step: TestStep = serde_yaml::from_str(yaml).unwrap();
    let scroll = step.scroll.unwrap();
    assert!(scroll.selector.is_none());
    assert!(scroll.x.is_none());
    assert!(scroll.y.is_none());
}

#[test]
fn test_step_hover() {
    let yaml = "hover: '.menu-item'";
    let step: TestStep = serde_yaml::from_str(yaml).unwrap();
    let target = step.hover.unwrap();
    assert_eq!(target.to_selector(), Some(".menu-item".into()));
}

#[test]
fn test_step_press() {
    let yaml = "press: Enter";
    let step: TestStep = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(step.press, Some("Enter".into()));
}

#[test]
fn test_step_wait_defaults() {
    let yaml = "wait: {}";
    let step: TestStep = serde_yaml::from_str(yaml).unwrap();
    let wait = step.wait.unwrap();
    assert!(wait.selector.is_none());
    assert!(wait.text.is_none());
    assert!(wait.element_text.is_none());
    assert_eq!(wait.timeout_ms, 5000);
    assert!(wait.network_idle.is_none());
}

#[test]
fn test_step_wait_custom() {
    let yaml = "wait:\n  selector: \"#loading\"\n  timeout_ms: 10000\n  network_idle: true\n";
    let step: TestStep = serde_yaml::from_str(yaml).unwrap();
    let wait = step.wait.unwrap();
    assert_eq!(wait.selector, Some("#loading".into()));
    assert_eq!(wait.timeout_ms, 10000);
    assert_eq!(wait.network_idle, Some(true));
}

#[test]
fn test_step_wait_text() {
    let yaml = "wait:\n  text: \"Ready\"\n  timeout_ms: 2500\n";
    let step: TestStep = serde_yaml::from_str(yaml).unwrap();
    let wait = step.wait.unwrap();
    assert_eq!(wait.text, Some("Ready".into()));
    assert_eq!(wait.timeout_ms, 2500);
}

#[test]
fn test_step_wait_element_text() {
    let yaml = "wait:\n  element_text:\n    selector: \"#status\"\n    contains: \"Loaded\"\n";
    let step: TestStep = serde_yaml::from_str(yaml).unwrap();
    let wait = step.wait.unwrap();
    let element_text = wait.element_text.unwrap();
    assert_eq!(element_text.selector, "#status");
    assert_eq!(element_text.contains, "Loaded");
}

#[test]
fn test_step_batch() {
    let yaml = concat!(
        "batch:\n",
        "  - action: click\n    selector: \"#btn1\"\n",
        "  - action: type\n    selector: \"#input1\"\n    value: \"hello\"\n",
        "  - action: navigate\n    url: \"http://example.com/next\"\n",
    );
    let step: TestStep = serde_yaml::from_str(yaml).unwrap();
    let batch = step.batch.unwrap();
    assert_eq!(batch.len(), 3);
    assert_eq!(batch[0].action, "click");
    assert_eq!(batch[0].selector, Some("#btn1".into()));
    assert_eq!(batch[1].action, "type");
    assert_eq!(batch[1].value, Some("hello".into()));
    assert_eq!(batch[2].action, "navigate");
    assert_eq!(batch[2].url, Some("http://example.com/next".into()));
}

#[test]
fn test_step_cookie_action() {
    let yaml = "cookie:\n  action: set\n  browser: firefox\n  domain: example.com\n";
    let step: TestStep = serde_yaml::from_str(yaml).unwrap();
    let cookie = step.cookie.unwrap();
    assert_eq!(cookie.action, "set");
    assert_eq!(cookie.browser, Some("firefox".into()));
    assert_eq!(cookie.domain, Some("example.com".into()));
}

#[test]
fn test_step_network_mock_action() {
    let yaml = concat!(
        "network_mock:\n",
        "  url_pattern: \"*/api/users*\"\n",
        "  response:\n",
        "    status: 201\n",
        "    body: '{\"ok\":true}'\n",
        "    content_type: application/json\n",
        "    headers:\n",
        "      x-test: mocked\n",
    );
    let step: TestStep = serde_yaml::from_str(yaml).unwrap();
    let mock = step.network_mock.unwrap();
    assert_eq!(mock.url_pattern, "*/api/users*");
    assert_eq!(mock.response.status, 201);
    assert_eq!(mock.response.body, "{\"ok\":true}");
    assert_eq!(
        mock.response.headers.unwrap().get("x-test"),
        Some(&"mocked".to_string())
    );
    assert_eq!(
        mock.response.content_type.as_deref(),
        Some("application/json")
    );
}

// ---------------------------------------------------------------------------
// SelectorTarget
// ---------------------------------------------------------------------------

#[test]
fn test_selector_target_string_to_selector() {
    let target = SelectorTarget::Selector("#foo".into());
    assert_eq!(target.to_selector(), Some("#foo".into()));
}

#[test]
fn test_selector_target_structured_with_selector() {
    let target = SelectorTarget::Structured {
        selector: Some(".bar".into()),
        id: Some(42),
        text: Some("Click me".into()),
    };
    assert_eq!(target.to_selector(), Some(".bar".into()));
}

#[test]
fn test_selector_target_structured_without_selector() {
    let target = SelectorTarget::Structured {
        selector: None,
        id: Some(5),
        text: Some("OK".into()),
    };
    assert_eq!(target.to_selector(), None);
}

// ---------------------------------------------------------------------------
// Assertion variants
// ---------------------------------------------------------------------------

#[test]
fn test_assertion_page_map_contains_yaml() {
    let yaml = "- page_map_contains:\n    tag: button\n    text: Submit\n";
    let assertions: Vec<Assertion> = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(assertions.len(), 1);
    let pmc = assertions[0].page_map_contains.as_ref().unwrap();
    assert_eq!(pmc.tag, Some("button".into()));
    assert_eq!(pmc.text, Some("Submit".into()));
}

#[test]
fn test_assertion_text_contains_yaml() {
    let yaml = "- text_contains: \"Welcome\"\n";
    let assertions: Vec<Assertion> = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(assertions[0].text_contains, Some("Welcome".into()));
}

#[test]
fn test_assertion_screenshot_yaml() {
    let yaml = "- screenshot:\n    name: homepage\n    threshold: 0.05\n    full_page: true\n";
    let assertions: Vec<Assertion> = serde_yaml::from_str(yaml).unwrap();
    let ss = assertions[0].screenshot.as_ref().unwrap();
    assert_eq!(ss.name, "homepage");
    assert!((ss.threshold - 0.05).abs() < 0.001);
    assert!(ss.full_page);
    assert!(ss.selector.is_none());
}

#[test]
fn test_assertion_screenshot_defaults() {
    let yaml = "- screenshot:\n    name: test\n";
    let assertions: Vec<Assertion> = serde_yaml::from_str(yaml).unwrap();
    let ss = assertions[0].screenshot.as_ref().unwrap();
    assert!((ss.threshold - 0.01).abs() < 0.001);
    assert!(!ss.full_page);
}

#[test]
fn test_assertion_network_called_yaml() {
    let yaml = "- network_called:\n    url: \"/api/users\"\n    method: POST\n";
    let assertions: Vec<Assertion> = serde_yaml::from_str(yaml).unwrap();
    let nc = assertions[0].network_called.as_ref().unwrap();
    assert_eq!(nc.url, "/api/users");
    assert_eq!(nc.method, Some("POST".into()));
}

// ---------------------------------------------------------------------------
// BatchStepAction
// ---------------------------------------------------------------------------

#[test]
fn test_batch_step_action_defaults() {
    let yaml = "action: click";
    let bsa: BatchStepAction = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(bsa.action, "click");
    assert!(bsa.selector.is_none());
    assert!(bsa.id.is_none());
    assert!(bsa.value.is_none());
    assert!(bsa.url.is_none());
    assert!(bsa.key.is_none());
}

#[test]
fn test_batch_step_action_full() {
    let yaml = "action: type\nselector: \"#input\"\nid: 7\nvalue: \"hello world\"\nkey: Enter\n";
    let bsa: BatchStepAction = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(bsa.action, "type");
    assert_eq!(bsa.selector, Some("#input".into()));
    assert_eq!(bsa.id, Some(7));
    assert_eq!(bsa.value, Some("hello world".into()));
    assert_eq!(bsa.key, Some("Enter".into()));
}

// ---------------------------------------------------------------------------
// RayoConfig
// ---------------------------------------------------------------------------

#[test]
fn test_rayo_config_default() {
    let config = RayoConfig::default();
    assert!(config.base_url.is_none());
}

#[test]
fn test_rayo_config_yaml() {
    let yaml = "base_url: http://localhost:3000";
    let config: RayoConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.base_url, Some("http://localhost:3000".into()));
}

#[test]
fn test_rayo_config_empty_yaml() {
    let yaml = "{}";
    let config: RayoConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(config.base_url.is_none());
}

#[test]
fn test_rayo_config_clone() {
    let config = RayoConfig {
        base_url: Some("http://localhost:8080".into()),
    };
    let cloned = config.clone();
    assert_eq!(cloned.base_url, config.base_url);
}

// ---------------------------------------------------------------------------
// Full suite roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_full_suite_roundtrip_complex() {
    let yaml = concat!(
        "name: \"Complex Suite\"\n",
        "viewport:\n  width: 1440\n  height: 900\n",
        "setup:\n  - navigate: \"http://example.com\"\n",
        "  - wait:\n      network_idle: true\n      timeout_ms: 3000\n",
        "steps:\n",
        "  - name: \"Type email\"\n    type:\n      selector: \"#email\"\n      value: \"user@test.com\"\n",
        "  - name: \"Select country\"\n    select:\n      selector: \"#country\"\n      value: \"US\"\n",
        "  - name: \"Submit form\"\n    click: \"#submit\"\n    assert:\n      - text_contains: \"Success\"\n",
        "teardown:\n  - navigate: \"http://example.com/logout\"\n",
    );
    let suite: TestSuite = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(suite.name, "Complex Suite");
    assert_eq!(suite.setup.len(), 2);
    assert_eq!(suite.steps.len(), 3);
    assert_eq!(suite.teardown.len(), 1);
    let submit_step = &suite.steps[2];
    let assertions = submit_step.assert.as_ref().unwrap();
    assert_eq!(assertions.len(), 1);
    assert_eq!(assertions[0].text_contains, Some("Success".into()));
    let serialized = serde_yaml::to_string(&suite).unwrap();
    let reparsed: TestSuite = serde_yaml::from_str(&serialized).unwrap();
    assert_eq!(reparsed.name, suite.name);
    assert_eq!(reparsed.steps.len(), suite.steps.len());
}

// ---------------------------------------------------------------------------
// All None assertion
// ---------------------------------------------------------------------------

#[test]
fn test_assertion_all_none_yaml() {
    let yaml = "{}";
    let assertion: Assertion = serde_yaml::from_str(yaml).unwrap();
    assert!(assertion.page_map_contains.is_none());
    assert!(assertion.text_contains.is_none());
    assert!(assertion.screenshot.is_none());
    assert!(assertion.network_called.is_none());
}

#[test]
fn test_page_map_assertion_all_fields() {
    let yaml = "selector: \"div.card\"\ntext: \"Hello\"\nrole: \"button\"\ntag: \"div\"\n";
    let pma: PageMapAssertion = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(pma.selector, Some("div.card".into()));
    assert_eq!(pma.text, Some("Hello".into()));
    assert_eq!(pma.role, Some("button".into()));
    assert_eq!(pma.tag, Some("div".into()));
}

#[test]
fn test_network_assertion_no_method() {
    let yaml = "url: \"/api/data\"\n";
    let na: NetworkAssertion = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(na.url, "/api/data");
    assert!(na.method.is_none());
}

#[test]
fn test_screenshot_assertion_with_selector() {
    let yaml = "name: card-component\nselector: \".card\"\nthreshold: 0.02\n";
    let ss: ScreenshotAssertion = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(ss.name, "card-component");
    assert_eq!(ss.selector, Some(".card".into()));
    assert!((ss.threshold - 0.02).abs() < 0.001);
    assert!(!ss.full_page);
}
