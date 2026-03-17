# Page Map Metadata: Element State + Truncation Info

## Why
AI agents using page_map cannot distinguish disabled/readonly/required form elements from active ones, leading to wasted actions (clicking disabled buttons, skipping required fields). When page_map truncates at MAX_ELEMENTS (50), the agent has no indication that elements were dropped.

## Solution
Add two metadata enhancements to page_map, both designed to be token-efficient (only present when non-empty/relevant):

1. **Element state** — a `state` array on `InteractiveElement` reporting `disabled`, `readonly`, `required`, `checked`, `hidden`. Skipped when empty (zero extra tokens for normal elements).
2. **Truncation metadata** — `total_interactive` and `truncated` fields on `PageMap`, only serialized when the MAX_ELEMENTS cap was hit. Tells the agent how many elements exist vs. how many were returned.

## Scope

### rayo-core
- `page_map.rs` — add `state: Vec<String>` to `InteractiveElement`, add `total_interactive: Option<usize>` and `truncated: Option<bool>` to `PageMap`, update `EXTRACT_PAGE_MAP_JS` to detect element states and count total elements before truncation
- `browser.rs` — update the scoped page_map JS (selector-based subtree extraction) with the same state detection and truncation metadata logic

### Boundary
- Token-efficient: new fields use `skip_serializing_if` so they add zero tokens when not applicable
- No behavioral changes to element extraction or ordering
- Both full and scoped page_map variants get the same enhancements

## Not in scope
- Additional element states (e.g., `aria-expanded`, `aria-selected`)
- Dynamic truncation limits based on context window
- Element state filtering (e.g., excluding disabled elements from the map)
