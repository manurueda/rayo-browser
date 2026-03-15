//! Token-efficient page representation for LLMs.
//!
//! Instead of screenshots (100k tokens) or raw HTML (50k tokens),
//! produces a structured JSON map of the page in ~500 tokens.
//!
//! AI agents read this to understand the page and reference elements by ID.

use serde::{Deserialize, Serialize};

/// A compact, token-efficient representation of a web page.
///
/// Designed for LLM consumption: every interactive element gets
/// a numeric `id` that can be used in batch actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageMap {
    pub url: String,
    pub title: String,
    /// All interactive elements (inputs, buttons, links, selects).
    pub interactive: Vec<InteractiveElement>,
    /// Page headings for context.
    pub headings: Vec<String>,
    /// Short text summary of the page content.
    pub text_summary: String,
}

/// An interactive element on the page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveElement {
    /// Stable numeric ID for referencing in actions.
    pub id: usize,
    /// HTML tag name.
    pub tag: String,
    /// Input type (for inputs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    /// Name attribute.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Associated label text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Visible text content (for buttons, links).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Placeholder text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// Current value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Available options (for selects, radios).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
    /// ARIA role.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// href for links.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    /// CSS selector to locate this element.
    #[serde(skip_serializing)]
    pub selector: String,
}

/// JavaScript to extract the page map from the browser.
/// Runs as a single Runtime.evaluate — one CDP round-trip.
pub const EXTRACT_PAGE_MAP_JS: &str = r#"
(() => {
    const interactive = [];
    const selectors = 'a[href], button, input, select, textarea, [role="button"], [role="link"], [role="tab"], [onclick]';
    const elements = document.querySelectorAll(selectors);

    const MAX_ELEMENTS = 50;
    let count = 0;
    elements.forEach((el, idx) => {
        if (count >= MAX_ELEMENTS) return;
        if (el.offsetParent === null && el.type !== 'hidden') return; // Skip invisible

        const item = { id: idx, tag: el.tagName.toLowerCase(), selector: '' };

        // Type
        if (el.type) item.type = el.type;

        // Name
        if (el.name) item.name = el.name;

        // Label
        const labelEl = el.labels && el.labels[0];
        if (labelEl) {
            item.label = labelEl.textContent.trim();
        } else if (el.getAttribute('aria-label')) {
            item.label = el.getAttribute('aria-label');
        } else if (el.placeholder) {
            item.label = el.placeholder;
        }

        // Text content (for buttons, links)
        const text = el.textContent?.trim();
        if (text && text.length < 100 && (el.tagName === 'BUTTON' || el.tagName === 'A')) {
            item.text = text;
        }

        // Placeholder
        if (el.placeholder) item.placeholder = el.placeholder;

        // Value
        if (el.value && el.type !== 'password') item.value = el.value;

        // Options (select)
        if (el.tagName === 'SELECT') {
            item.options = Array.from(el.options).map(o => o.text || o.value);
        }

        // Radio/checkbox values (group)
        if (el.type === 'radio' || el.type === 'checkbox') {
            const group = document.querySelectorAll(`input[name="${el.name}"]`);
            if (group.length > 1) {
                item.options = Array.from(group).map(r => r.value);
            }
        }

        // Role
        const role = el.getAttribute('role');
        if (role) item.role = role;

        // Href (links) — truncate long URLs
        if (el.href) item.href = el.href.length > 120 ? el.href.slice(0, 120) : el.href;

        // Build a reliable selector
        if (el.id) {
            item.selector = '#' + CSS.escape(el.id);
        } else if (el.name) {
            item.selector = `${el.tagName.toLowerCase()}[name="${el.name}"]`;
        } else {
            // Nth-of-type fallback
            const parent = el.parentElement;
            if (parent) {
                const siblings = parent.querySelectorAll(':scope > ' + el.tagName.toLowerCase());
                const index = Array.from(siblings).indexOf(el) + 1;
                item.selector = `${el.tagName.toLowerCase()}:nth-of-type(${index})`;
            }
        }

        interactive.push(item);
        count++;
    });

    // Headings
    const headings = Array.from(document.querySelectorAll('h1, h2, h3'))
        .map(h => h.textContent.trim())
        .filter(t => t.length > 0)
        .slice(0, 10);

    // Text summary (first meaningful paragraph)
    const paragraphs = Array.from(document.querySelectorAll('p'))
        .map(p => p.textContent.trim())
        .filter(t => t.length > 20);
    const textSummary = paragraphs.slice(0, 2).join(' ').slice(0, 300);

    return {
        url: window.location.href,
        title: document.title,
        interactive: interactive,
        headings: headings,
        text_summary: textSummary || document.title,
    };
})()
"#;

impl PageMap {
    /// Estimate the token count for this page map.
    /// Rough estimate: ~4 chars per token for JSON.
    pub fn estimated_tokens(&self) -> usize {
        let json = serde_json::to_string(self).unwrap_or_default();
        json.len() / 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_map_serialization() {
        let map = PageMap {
            url: "https://example.com".into(),
            title: "Example".into(),
            interactive: vec![InteractiveElement {
                id: 0,
                tag: "input".into(),
                r#type: Some("text".into()),
                name: Some("q".into()),
                label: Some("Search".into()),
                text: None,
                placeholder: Some("Search...".into()),
                value: None,
                options: None,
                role: None,
                href: None,
                selector: "input[name=\"q\"]".into(),
            }],
            headings: vec!["Welcome".into()],
            text_summary: "A simple example page.".into(),
        };

        let json = serde_json::to_string_pretty(&map).unwrap();
        assert!(json.contains("interactive"));
        assert!(!json.contains("selector")); // selector is skip_serializing
        assert!(map.estimated_tokens() < 200);
    }
}
