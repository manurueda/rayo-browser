//! User story types — parsed from `*.story.yaml` files.
//!
//! A story chains multiple test flows into an end-to-end user journey.
//! Stories are discoverable by the auto-discover pipeline and readable
//! by non-developers.

use crate::types::Assertion;
use serde::{Deserialize, Serialize};

/// A user story loaded from a `*.story.yaml` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStory {
    /// Human-readable name, e.g. "Customer purchases a product".
    pub name: String,
    /// Plain-English description of the journey.
    #[serde(default)]
    pub description: String,
    /// Persona performing this journey (e.g. "customer", "admin", "guest").
    #[serde(default)]
    pub persona: Option<String>,
    /// Importance level: "critical", "high", "medium", "low".
    #[serde(default)]
    pub importance: Option<String>,
    /// Names of prerequisite stories that must pass first (shares browser session).
    #[serde(default)]
    pub requires: Vec<String>,
    /// Ordered list of flows in this story.
    pub flows: Vec<StoryFlow>,
    /// Tags for filtering (e.g. "checkout", "auth", "admin").
    #[serde(default)]
    pub tags: Vec<String>,
}

/// A flow within a story, referencing a TestSuite by name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryFlow {
    /// TestSuite name to execute (must match a `*.test.yaml` suite name).
    pub name: String,
    /// Human-readable post-conditions checked after this flow completes.
    #[serde(default)]
    pub then: Vec<StoryAssertion>,
}

/// A human-readable assertion with a description and machine-checkable assert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryAssertion {
    /// Plain-English description shown in dashboard (e.g. "User sees dashboard").
    pub description: String,
    /// Machine-checkable assertion (reuses existing Assertion type).
    #[serde(default)]
    pub assert: Option<Assertion>,
}

/// Result of running a single story.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryResult {
    /// Story name.
    pub name: String,
    /// Story description.
    pub description: String,
    /// Persona.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persona: Option<String>,
    /// Importance level.
    pub importance: String,
    /// Whether the entire story passed.
    pub passed: bool,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
    /// Per-flow results.
    pub flow_results: Vec<StoryFlowResult>,
    /// Error message if story failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// ISO 8601 timestamp when the story started.
    pub started_at: String,
}

/// Result of a single flow within a story.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryFlowResult {
    /// Flow name (TestSuite name).
    pub flow_name: String,
    /// Whether all steps + then-assertions passed.
    pub passed: bool,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Per-step results from the test runner.
    pub steps_passed: usize,
    pub steps_total: usize,
    /// Human-readable then-assertion results.
    pub then_results: Vec<StoryAssertionResult>,
    /// Error message if flow failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Result of a single then-assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryAssertionResult {
    /// Human-readable description (from StoryAssertion).
    pub description: String,
    /// Whether the assertion passed.
    pub passed: bool,
    /// Error details if failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_story_yaml_roundtrip() {
        let yaml = r#"
name: "Customer purchases a product"
description: "End-to-end checkout flow"
persona: customer
importance: critical
requires:
  - "login Login Flow"
flows:
  - name: "home Search"
    then:
      - description: "Search results appear"
        assert:
          page_map_contains:
            tag: a
  - name: "checkout Form"
    then:
      - description: "Order confirmed"
        assert:
          text_contains: "Order confirmed"
tags:
  - checkout
  - e2e
"#;

        let story: UserStory = serde_yaml::from_str(yaml).expect("Should parse story YAML");
        assert_eq!(story.name, "Customer purchases a product");
        assert_eq!(story.persona, Some("customer".into()));
        assert_eq!(story.importance, Some("critical".into()));
        assert_eq!(story.requires.len(), 1);
        assert_eq!(story.flows.len(), 2);
        assert_eq!(story.flows[0].then.len(), 1);
        assert_eq!(story.flows[0].then[0].description, "Search results appear");
        assert_eq!(story.tags, vec!["checkout", "e2e"]);

        // Roundtrip
        let serialized = serde_yaml::to_string(&story).expect("Should serialize");
        let reparsed: UserStory = serde_yaml::from_str(&serialized).expect("Should reparse");
        assert_eq!(reparsed.name, story.name);
        assert_eq!(reparsed.flows.len(), story.flows.len());
    }

    #[test]
    fn test_minimal_story_yaml() {
        let yaml = r#"
name: "Simple test"
flows:
  - name: "Smoke Test"
"#;
        let story: UserStory = serde_yaml::from_str(yaml).expect("Should parse minimal story");
        assert_eq!(story.name, "Simple test");
        assert!(story.description.is_empty());
        assert!(story.persona.is_none());
        assert!(story.importance.is_none());
        assert!(story.requires.is_empty());
        assert_eq!(story.flows.len(), 1);
        assert!(story.flows[0].then.is_empty());
        assert!(story.tags.is_empty());
    }

    #[test]
    fn test_story_with_no_console_errors() {
        let yaml = r#"
name: "Error-free browsing"
flows:
  - name: "home Navigation"
    then:
      - description: "No JavaScript errors"
        assert:
          no_console_errors: true
"#;
        let story: UserStory = serde_yaml::from_str(yaml).expect("Should parse");
        let then_assert = &story.flows[0].then[0];
        assert_eq!(then_assert.description, "No JavaScript errors");
        let assertion = then_assert.assert.as_ref().expect("Should have assertion");
        assert_eq!(assertion.no_console_errors, Some(true));
    }
}
