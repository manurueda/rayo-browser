//! Element inspection: computed styles, box model, applied rules, diagnostics.
//!
//! Provides a "DevTools Elements panel" equivalent for AI agents.
//! Returns structured CSS/layout/accessibility data per element,
//! far more reliable and token-efficient than screenshots for CSS verification.
//!
//! ```text
//! Agent → rayo_observe mode=inspect id=3
//!   → computed styles (curated ~40 or all ~350)
//!   → applied CSS rules with source file/line
//!   → CSS variable resolution chains
//!   → box model (margin/border/padding/content)
//!   → accessibility (role/name/states)
//!   → visibility diagnosis (causal)
//!   → layout anomaly warnings
//!   → style diff (opt-in)
//!   → expect assertions (opt-in)
//! ```

use std::collections::HashMap;
use std::num::NonZeroUsize;

use lru::LruCache;
use serde::{Deserialize, Serialize};

// ── Curated property list ──────────────────────────────────────────

/// The ~40 most diagnostic CSS properties, covering layout, color, visibility,
/// spacing, typography, and positioning. These are the properties that matter
/// when debugging "why does this element look wrong?"
pub const CURATED_PROPERTIES: &[&str] = &[
    // Layout
    "display",
    "position",
    "float",
    "clear",
    "flex-direction",
    "flex-wrap",
    "align-items",
    "justify-content",
    "grid-template-columns",
    "grid-template-rows",
    // Dimensions
    "width",
    "height",
    "min-width",
    "min-height",
    "max-width",
    "max-height",
    "box-sizing",
    // Spacing
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    // Color
    "color",
    "background-color",
    "background-image",
    "border-color",
    "border-style",
    "border-width",
    // Visibility & stacking
    "opacity",
    "visibility",
    "overflow",
    "overflow-x",
    "overflow-y",
    "z-index",
    "pointer-events",
    // Typography
    "font-family",
    "font-size",
    "font-weight",
    "line-height",
    "text-align",
    // Transform
    "transform",
];

/// Category shorthands that expand to sets of property names.
pub fn expand_category(category: &str) -> Option<&'static [&'static str]> {
    match category {
        "layout" => Some(&[
            "display",
            "position",
            "float",
            "clear",
            "flex-direction",
            "flex-wrap",
            "align-items",
            "justify-content",
            "grid-template-columns",
            "grid-template-rows",
            "width",
            "height",
            "min-width",
            "min-height",
            "max-width",
            "max-height",
            "box-sizing",
        ]),
        "color" => Some(&[
            "color",
            "background-color",
            "background-image",
            "border-color",
            "border-style",
            "border-width",
            "opacity",
        ]),
        "visibility" => Some(&[
            "display",
            "visibility",
            "opacity",
            "overflow",
            "overflow-x",
            "overflow-y",
            "z-index",
            "pointer-events",
        ]),
        "spacing" => Some(&[
            "margin-top",
            "margin-right",
            "margin-bottom",
            "margin-left",
            "padding-top",
            "padding-right",
            "padding-bottom",
            "padding-left",
        ]),
        "typography" => Some(&[
            "font-family",
            "font-size",
            "font-weight",
            "line-height",
            "text-align",
            "color",
        ]),
        _ => None,
    }
}

/// Resolve a list of property names from user input (individual names + categories).
pub fn resolve_properties(input: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in input {
        if let Some(expanded) = expand_category(item) {
            result.extend(expanded.iter().map(|s| s.to_string()));
        } else {
            result.push(item.clone());
        }
    }
    result.sort();
    result.dedup();
    result
}

// ── Result types ──────────────────────────────────────────────────

/// Target element metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectTarget {
    pub selector: String,
    pub tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<usize>,
}

/// Visibility diagnosis result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisibilityInfo {
    pub visible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnosis: Option<String>,
}

/// A single applied CSS rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedRule {
    pub selector: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub properties: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub specificity: Option<[u32; 3]>,
}

/// CSS variable resolution chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableChain {
    pub chain: Vec<String>,
    pub resolved: String,
}

/// Box model dimensions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoxModel {
    pub content: [f64; 2],
    pub padding: [f64; 4],
    pub border: [f64; 4],
    pub margin: [f64; 4],
}

/// Accessibility info for an element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibilityInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub focusable: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<String>,
}

/// Result of an expect assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectResult {
    pub property: String,
    pub expected: String,
    pub actual: String,
    pub pass: bool,
}

/// Style diff between two inspect calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleDiff {
    pub changed: HashMap<String, StyleChange>,
}

/// A single property change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleChange {
    pub before: String,
    pub after: String,
}

/// Complete result of inspecting an element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectResult {
    pub target: InspectTarget,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anomalies: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<VisibilityInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computed: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub box_model: Option<BoxModel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applied_rules: Option<Vec<AppliedRule>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<HashMap<String, VariableChain>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessibility: Option<AccessibilityInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<StyleDiff>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expect_results: Option<Vec<ExpectResult>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl InspectResult {
    pub fn estimated_tokens(&self) -> usize {
        let json = serde_json::to_string(self).unwrap_or_default();
        json.len() / 4
    }
}

/// Options for inspect_element.
#[derive(Debug, Clone, Default)]
pub struct InspectOptions {
    /// Specific properties to return (individual names or category shorthands).
    pub properties: Option<Vec<String>>,
    /// Return all ~350 computed properties.
    pub all: bool,
    /// Stripped response (~200 tokens) — omits applied_rules, variables, accessibility.
    pub compact: bool,
    /// Include before/after diff from cached previous.
    pub diff: bool,
    /// Assert expected values per property.
    pub expect: Option<HashMap<String, String>>,
}

// ── Diff cache (survives DOM mutations) ────────────────────────────

/// Cache for style diff. Keyed by selector, NOT invalidated on DOM mutations.
/// Its purpose is to store the pre-mutation state for comparison.
pub struct DiffCache {
    cache: LruCache<String, HashMap<String, String>>,
}

impl DiffCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: LruCache::new(
                NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(256).unwrap()),
            ),
        }
    }

    /// Get the previously cached computed styles for an element.
    pub fn get(&mut self, selector: &str) -> Option<&HashMap<String, String>> {
        self.cache.get(selector)
    }

    /// Store computed styles for future diff.
    pub fn put(&mut self, selector: String, styles: HashMap<String, String>) {
        self.cache.put(selector, styles);
    }
}

// ── Variable chain resolution ─────────────────────────────────────

const MAX_VAR_DEPTH: usize = 10;

/// Resolve CSS variable chains from computed + matched rule data.
/// Input: a map of computed property values.
/// Returns: chains for any property whose value involves var() in matched rules.
pub fn resolve_variable_chains(
    matched_vars: &HashMap<String, String>,
    all_vars: &HashMap<String, String>,
) -> HashMap<String, VariableChain> {
    let mut result = HashMap::new();
    for (name, value) in matched_vars {
        if !name.starts_with("--") && !value.contains("var(") {
            continue;
        }
        let chain = resolve_single_chain(value, all_vars, 0);
        if chain.len() > 1 {
            let resolved = chain.last().cloned().unwrap_or_default();
            result.insert(name.clone(), VariableChain { chain, resolved });
        }
    }
    result
}

fn resolve_single_chain(
    value: &str,
    all_vars: &HashMap<String, String>,
    depth: usize,
) -> Vec<String> {
    if depth >= MAX_VAR_DEPTH {
        return vec![value.to_string(), "[circular or too deep]".to_string()];
    }

    let mut chain = vec![value.to_string()];

    // Extract var(--name) or var(--name, fallback)
    if let Some(inner) = extract_var_content(value) {
        let (var_name, fallback) = parse_var_args(inner);

        if let Some(resolved) = all_vars.get(var_name) {
            let sub = resolve_single_chain(resolved, all_vars, depth + 1);
            chain.extend(sub);
        } else if let Some(fb) = fallback {
            // Variable undefined, use fallback
            let sub = resolve_single_chain(fb, all_vars, depth + 1);
            chain.extend(sub);
        } else {
            chain.push("[undefined]".to_string());
        }
    }

    chain
}

/// Extract the content inside var(...).
fn extract_var_content(value: &str) -> Option<&str> {
    let start = value.find("var(")?;
    let inner_start = start + 4;
    // Find matching closing paren, accounting for nesting
    let bytes = value.as_bytes();
    let mut depth = 1;
    let mut pos = inner_start;
    while pos < bytes.len() && depth > 0 {
        match bytes[pos] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ => {}
        }
        if depth > 0 {
            pos += 1;
        }
    }
    if depth == 0 {
        Some(&value[inner_start..pos])
    } else {
        None
    }
}

/// Parse "var()" args: "--name" or "--name, fallback".
fn parse_var_args(inner: &str) -> (&str, Option<&str>) {
    if let Some(comma_pos) = inner.find(',') {
        let name = inner[..comma_pos].trim();
        let fallback = inner[comma_pos + 1..].trim();
        (name, Some(fallback))
    } else {
        (inner.trim(), None)
    }
}

// ── JavaScript for diagnostics ────────────────────────────────────

/// Batched JS that runs visibility diagnosis, layout anomaly detection,
/// and collects event listener info for a single element.
/// Takes a CSS selector as the argument.
pub const INSPECT_DIAGNOSTICS_JS: &str = r#"
((selector) => {
    const el = document.querySelector(selector);
    if (!el) return { error: 'Element not found' };

    const result = {
        tag: el.tagName.toLowerCase(),
        visibility: { visible: true, diagnosis: null },
        anomalies: [],
    };

    // --- Visibility diagnosis ---
    const cs = getComputedStyle(el);
    if (cs.display === 'none') {
        result.visibility = { visible: false, diagnosis: 'display: none on element itself' };
    } else if (cs.visibility === 'hidden') {
        result.visibility = { visible: false, diagnosis: 'visibility: hidden on element itself' };
    } else if (parseFloat(cs.opacity) === 0) {
        result.visibility = { visible: false, diagnosis: 'opacity: 0 on element itself' };
    } else {
        // Walk ancestors for hidden containers
        let ancestor = el.parentElement;
        let depth = 0;
        while (ancestor && depth < 50) {
            const acs = getComputedStyle(ancestor);
            if (acs.display === 'none') {
                const asel = ancestor.tagName.toLowerCase() +
                    (ancestor.className ? '.' + ancestor.className.split(' ')[0] : '');
                result.visibility = { visible: false,
                    diagnosis: 'hidden by display:none on ancestor ' + asel };
                break;
            }
            if (acs.visibility === 'hidden') {
                const asel = ancestor.tagName.toLowerCase() +
                    (ancestor.className ? '.' + ancestor.className.split(' ')[0] : '');
                result.visibility = { visible: false,
                    diagnosis: 'hidden by visibility:hidden on ancestor ' + asel };
                break;
            }
            if (parseFloat(acs.opacity) === 0) {
                const asel = ancestor.tagName.toLowerCase() +
                    (ancestor.className ? '.' + ancestor.className.split(' ')[0] : '');
                result.visibility = { visible: false,
                    diagnosis: 'hidden by opacity:0 on ancestor ' + asel };
                break;
            }
            ancestor = ancestor.parentElement;
            depth++;
        }
    }

    // Check if offscreen
    const rect = el.getBoundingClientRect();
    if (result.visibility.visible && rect.width > 0 && rect.height > 0) {
        const vw = window.innerWidth;
        const vh = window.innerHeight;
        if (rect.right < 0 || rect.bottom < 0 || rect.left > vw || rect.top > vh) {
            result.visibility = { visible: false,
                diagnosis: 'off-screen at (' + Math.round(rect.left) + ',' + Math.round(rect.top) + ')' };
        }
    }

    // Check overflow clipping on ancestors
    if (result.visibility.visible) {
        let clipAncestor = el.parentElement;
        let cd = 0;
        while (clipAncestor && cd < 30) {
            const acs = getComputedStyle(clipAncestor);
            if (acs.overflow === 'hidden' || acs.overflowX === 'hidden' || acs.overflowY === 'hidden') {
                const ar = clipAncestor.getBoundingClientRect();
                if (rect.right < ar.left || rect.bottom < ar.top ||
                    rect.left > ar.right || rect.top > ar.bottom) {
                    const asel = clipAncestor.tagName.toLowerCase() +
                        (clipAncestor.className ? '.' + clipAncestor.className.split(' ')[0] : '');
                    result.visibility = { visible: false,
                        diagnosis: 'clipped by overflow:hidden on ancestor ' + asel };
                    break;
                }
            }
            clipAncestor = clipAncestor.parentElement;
            cd++;
        }
    }

    // --- Layout anomaly detection ---
    if (rect.width === 0 && rect.height === 0 && cs.display !== 'none') {
        result.anomalies.push('zero dimensions (0x0)');
    }
    if (rect.width === 0 && rect.height > 0) {
        result.anomalies.push('zero width');
    }
    if (rect.height === 0 && rect.width > 0) {
        result.anomalies.push('zero height');
    }
    const vw2 = window.innerWidth;
    const vh2 = window.innerHeight;
    if (rect.right > vw2 + 10) {
        result.anomalies.push('overflows viewport right by ' + Math.round(rect.right - vw2) + 'px');
    }
    if (rect.bottom > vh2 + 100) {
        result.anomalies.push('extends ' + Math.round(rect.bottom - vh2) + 'px below viewport');
    }

    // Check semi-transparent background (the dot bleed-through issue)
    const bg = cs.backgroundColor;
    if (bg && bg.startsWith('rgba')) {
        const alpha = parseFloat(bg.split(',')[3]);
        if (alpha > 0 && alpha < 0.5) {
            result.anomalies.push('semi-transparent background (alpha: ' + alpha.toFixed(2) + ') — content behind may bleed through');
        }
    }

    return result;
})
"#;

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_curated_properties_count() {
        assert!(CURATED_PROPERTIES.len() >= 35);
        assert!(CURATED_PROPERTIES.len() <= 50);
    }

    #[test]
    fn test_expand_category() {
        let layout = expand_category("layout").unwrap();
        assert!(layout.contains(&"display"));
        assert!(layout.contains(&"position"));

        let color = expand_category("color").unwrap();
        assert!(color.contains(&"background-color"));
        assert!(color.contains(&"opacity"));

        assert!(expand_category("nonexistent").is_none());
    }

    #[test]
    fn test_resolve_properties_mixed() {
        let input = vec!["layout".into(), "z-index".into(), "color".into()];
        let resolved = resolve_properties(&input);
        assert!(resolved.contains(&"display".to_string()));
        assert!(resolved.contains(&"z-index".to_string()));
        // "color" as category expands to include background-color
        assert!(resolved.contains(&"background-color".to_string()));
        // No duplicates
        let mut deduped = resolved.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(resolved.len(), deduped.len());
    }

    #[test]
    fn test_variable_chain_simple() {
        let mut all_vars = HashMap::new();
        all_vars.insert("--primary".to_string(), "#007bff".to_string());

        let mut matched = HashMap::new();
        matched.insert("color".to_string(), "var(--primary)".to_string());

        let chains = resolve_variable_chains(&matched, &all_vars);
        let chain = chains.get("color").unwrap();
        assert_eq!(chain.chain, vec!["var(--primary)", "#007bff"]);
        assert_eq!(chain.resolved, "#007bff");
    }

    #[test]
    fn test_variable_chain_nested() {
        let mut all_vars = HashMap::new();
        all_vars.insert("--bg".to_string(), "var(--ds-bg-primary)".to_string());
        all_vars.insert("--ds-bg-primary".to_string(), "#0f172a".to_string());

        let mut matched = HashMap::new();
        matched.insert("background".to_string(), "var(--bg)".to_string());

        let chains = resolve_variable_chains(&matched, &all_vars);
        let chain = chains.get("background").unwrap();
        assert_eq!(
            chain.chain,
            vec!["var(--bg)", "var(--ds-bg-primary)", "#0f172a"]
        );
        assert_eq!(chain.resolved, "#0f172a");
    }

    #[test]
    fn test_variable_chain_circular() {
        let mut all_vars = HashMap::new();
        all_vars.insert("--a".to_string(), "var(--b)".to_string());
        all_vars.insert("--b".to_string(), "var(--a)".to_string());

        let mut matched = HashMap::new();
        matched.insert("color".to_string(), "var(--a)".to_string());

        let chains = resolve_variable_chains(&matched, &all_vars);
        let chain = chains.get("color").unwrap();
        // Should terminate with depth guard
        assert!(chain.resolved.contains("circular") || chain.resolved.contains("too deep"));
    }

    #[test]
    fn test_variable_chain_with_fallback() {
        let all_vars = HashMap::new(); // --missing is undefined

        let mut matched = HashMap::new();
        matched.insert("color".to_string(), "var(--missing, red)".to_string());

        let chains = resolve_variable_chains(&matched, &all_vars);
        let chain = chains.get("color").unwrap();
        assert_eq!(chain.resolved, "red");
    }

    #[test]
    fn test_extract_var_content() {
        assert_eq!(extract_var_content("var(--foo)"), Some("--foo"));
        assert_eq!(extract_var_content("var(--foo, bar)"), Some("--foo, bar"));
        assert_eq!(
            extract_var_content("var(--foo, var(--bar))"),
            Some("--foo, var(--bar)")
        );
        assert_eq!(extract_var_content("red"), None);
    }

    #[test]
    fn test_parse_var_args() {
        assert_eq!(parse_var_args("--foo"), ("--foo", None));
        assert_eq!(parse_var_args("--foo, red"), ("--foo", Some("red")));
        assert_eq!(
            parse_var_args("--foo, var(--bar)"),
            ("--foo", Some("var(--bar)"))
        );
    }

    #[test]
    fn test_diff_cache() {
        let mut cache = DiffCache::new(10);
        let mut styles = HashMap::new();
        styles.insert("color".to_string(), "red".to_string());
        cache.put("div.foo".to_string(), styles);

        let cached = cache.get("div.foo").unwrap();
        assert_eq!(cached.get("color").unwrap(), "red");

        assert!(cache.get("div.bar").is_none());
    }

    #[test]
    fn test_inspect_result_serialization() {
        let result = InspectResult {
            target: InspectTarget {
                selector: "div.panel".into(),
                tag: "div".into(),
                id: Some(3),
            },
            summary: "div.panel: visible, 400x300".into(),
            anomalies: vec![],
            visibility: Some(VisibilityInfo {
                visible: true,
                diagnosis: None,
            }),
            computed: Some(HashMap::from([(
                "background-color".into(),
                "#1a1a2e".into(),
            )])),
            box_model: None,
            applied_rules: None,
            variables: None,
            accessibility: None,
            diff: None,
            expect_results: None,
            warnings: vec![],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"summary\""));
        assert!(json.contains("div.panel"));
        assert!(!json.contains("\"warnings\"")); // empty vec is skipped
        assert!(!json.contains("\"box_model\"")); // None is skipped
    }
}
