//! Built-in default rules.

use std::collections::HashMap;

use crate::config::{RuleConfig, Severity};

/// Default rules shipped with rayo-browser.
pub fn default_rules() -> HashMap<String, RuleConfig> {
    let mut rules = HashMap::new();

    // Selector rules
    rules.insert(
        "selectors/prefer-css".into(),
        RuleConfig::Severity(Severity::Error),
    );
    rules.insert(
        "selectors/no-broad-selectors".into(),
        RuleConfig::Severity(Severity::Warn),
    );

    // Wait rules
    rules.insert(
        "waits/no-fixed-timeout".into(),
        RuleConfig::Severity(Severity::Error),
    );
    rules.insert(
        "waits/prefer-network-idle".into(),
        RuleConfig::Severity(Severity::Warn),
    );

    // Screenshot rules
    rules.insert(
        "screenshots/rate-limit".into(),
        RuleConfig::Full {
            severity: Severity::Warn,
            options: {
                let mut opts = HashMap::new();
                opts.insert(
                    "maxPerMinute".into(),
                    serde_json::Value::Number(10.into()),
                );
                opts
            },
        },
    );
    rules.insert(
        "screenshots/prefer-page-map".into(),
        RuleConfig::Severity(Severity::Warn),
    );

    // Batching rules
    rules.insert(
        "batching/combine-sequential".into(),
        RuleConfig::Severity(Severity::Warn),
    );

    // Context rules
    rules.insert(
        "contexts/prefer-reuse".into(),
        RuleConfig::Severity(Severity::Warn),
    );

    rules
}

/// Get rules as compact text for AI agent system prompts.
pub fn rules_as_agent_text() -> String {
    r#"RAYO SPEED RULES:
- PREFER CSS selectors over XPath (2-10x faster). Bad: "//div[@class='foo']" Good: "div.foo"
- DO NOT use broad selectors (*, div, span). Use classes, IDs, or data attributes.
- DO NOT use fixed timeouts (sleep, waitForTimeout). Use event-driven waits.
- DO NOT screenshot after every action. Max 10/min. Use rayo_observe page_map instead.
- PREFER page_map over screenshot for understanding page content (200x more token-efficient).
- BATCH 3+ sequential actions into rayo_batch tool (5-7x faster).
- REUSE browser contexts (creating costs 50-200ms each).
- USE element IDs from page_map in actions (e.g., {"action": "click", "id": 3})."#
        .to_string()
}
