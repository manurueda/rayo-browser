//! Violation → fix mapping.
//!
//! Maps detected slow patterns to concrete, actionable suggestions.

use crate::engine::Violation;

/// Enrich a violation with a more specific suggestion based on context.
pub fn enrich_suggestion(violation: &mut Violation, context: &str) {
    match violation.rule.as_str() {
        "selectors/prefer-css" => {
            if context.starts_with("//") {
                violation.suggestion = Some(format!(
                    "Convert XPath to CSS. Example: \"//div[@class='x']\" → \"div.x\". Your selector: {context}"
                ));
            }
        }
        "screenshots/rate-limit" => {
            violation.suggestion = Some(
                "Use rayo_observe with mode='page_map' instead. Returns structured page data in ~500 tokens vs ~100k for a screenshot.".into()
            );
        }
        "batching/combine-sequential" => {
            violation.suggestion = Some(
                "Use rayo_batch to combine multiple actions into one MCP call. Example: [{\"action\":\"click\",\"id\":1},{\"action\":\"type\",\"id\":2,\"value\":\"hello\"}]".into()
            );
        }
        _ => {}
    }
}
