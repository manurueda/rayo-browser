//! Test definition types — parsed from YAML test files.

use serde::{Deserialize, Serialize};

/// A test suite loaded from a YAML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSuite {
    /// Human-readable name.
    pub name: String,
    /// Viewport dimensions for this suite.
    #[serde(default)]
    pub viewport: Option<ViewportDef>,
    /// Setup steps run before any test case.
    #[serde(default)]
    pub setup: Vec<TestStep>,
    /// Test steps with assertions.
    pub steps: Vec<TestStep>,
    /// Teardown steps run after all tests.
    #[serde(default)]
    pub teardown: Vec<TestStep>,
}

/// Viewport configuration in test YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportDef {
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
}

fn default_width() -> u32 {
    1280
}
fn default_height() -> u32 {
    720
}

/// A single step in a test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestStep {
    /// Human-readable name for this step.
    #[serde(default)]
    pub name: Option<String>,

    // --- Actions (exactly one should be set) ---
    /// Navigate to a URL.
    #[serde(default)]
    pub navigate: Option<String>,
    /// Click an element.
    #[serde(default)]
    pub click: Option<SelectorTarget>,
    /// Type text into an element.
    #[serde(default)]
    pub r#type: Option<TypeAction>,
    /// Select an option.
    #[serde(default)]
    pub select: Option<SelectAction>,
    /// Scroll action.
    #[serde(default)]
    pub scroll: Option<ScrollAction>,
    /// Hover over an element.
    #[serde(default)]
    pub hover: Option<SelectorTarget>,
    /// Press a key.
    #[serde(default)]
    pub press: Option<String>,
    /// Wait for a condition.
    #[serde(default)]
    pub wait: Option<WaitAction>,
    /// Execute multiple actions in a batch.
    #[serde(default)]
    pub batch: Option<Vec<BatchStepAction>>,
    /// Import cookies.
    #[serde(default)]
    pub cookie: Option<CookieAction>,

    // --- Assertions (checked after action completes) ---
    #[serde(default)]
    pub assert: Option<Vec<Assertion>>,
}

/// Target an element by selector or text.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SelectorTarget {
    /// CSS selector string.
    Selector(String),
    /// Structured target with selector and optional text.
    Structured {
        selector: Option<String>,
        id: Option<usize>,
        text: Option<String>,
    },
}

impl SelectorTarget {
    pub fn to_selector(&self) -> Option<String> {
        match self {
            Self::Selector(s) => Some(s.clone()),
            Self::Structured { selector, .. } => selector.clone(),
        }
    }
}

/// Type text into an element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeAction {
    pub selector: String,
    pub value: String,
}

/// Select an option from a dropdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectAction {
    pub selector: String,
    pub value: String,
}

/// Scroll action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrollAction {
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default)]
    pub x: Option<i64>,
    #[serde(default)]
    pub y: Option<i64>,
}

/// Wait for a condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitAction {
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub network_idle: Option<bool>,
}

fn default_timeout() -> u64 {
    5000
}

/// Batch action within a step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchStepAction {
    pub action: String,
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default)]
    pub id: Option<usize>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
}

/// Cookie action in a step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieAction {
    pub action: String,
    #[serde(default)]
    pub browser: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
}

/// An assertion to check after a step.
/// Uses a flat struct with optional fields since serde_yaml
/// doesn't support internally tagged enums cleanly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assertion {
    /// Assert page map contains an element matching criteria.
    #[serde(default)]
    pub page_map_contains: Option<PageMapAssertion>,
    /// Assert page text contains a string.
    #[serde(default)]
    pub text_contains: Option<String>,
    /// Assert a screenshot matches a baseline.
    #[serde(default)]
    pub screenshot: Option<ScreenshotAssertion>,
    /// Assert a network request was made.
    #[serde(default)]
    pub network_called: Option<NetworkAssertion>,
}

/// Criteria for page map assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageMapAssertion {
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub tag: Option<String>,
}

/// Screenshot comparison assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotAssertion {
    /// Baseline name.
    pub name: String,
    /// Diff threshold (0.0-1.0). Default: 0.01.
    #[serde(default = "default_threshold")]
    pub threshold: f64,
    /// Whether to capture full page.
    #[serde(default)]
    pub full_page: bool,
    /// Element selector for element-level screenshot.
    #[serde(default)]
    pub selector: Option<String>,
}

fn default_threshold() -> f64 {
    0.01
}

/// Network request assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkAssertion {
    /// URL pattern to match.
    pub url: String,
    /// Expected HTTP method.
    #[serde(default)]
    pub method: Option<String>,
}

/// Project-level rayo configuration (`.rayo/config.yaml`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RayoConfig {
    /// Base URL prepended to relative navigate paths.
    #[serde(default)]
    pub base_url: Option<String>,
}
