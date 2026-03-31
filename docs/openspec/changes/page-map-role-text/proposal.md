# Page Map Role Text

## Why

The page map already captures visible text for native `<button>` and `<a>` elements, but it drops the same text for ARIA role equivalents such as `role="button"`, `role="link"`, and `role="tab"`. That makes custom component libraries look less descriptive to AI agents than equivalent native controls.

## Solution

Treat `role="button"`, `role="link"`, and `role="tab"` as text-bearing interactive elements when extracting `item.text`, using the same visibility and length guardrails already applied to native buttons and links.

## Scope

- `crates/rayo-core/src/page_map.rs`
- `crates/rayo-core/src/browser.rs`
- Focused integration coverage for full and scoped page maps

## Not in scope

- Additional ARIA roles beyond `button`, `link`, and `tab`
- Changing selector generation, ordering, or truncation behavior
- Changing label extraction semantics
