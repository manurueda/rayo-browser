# Design: Response Efficiency

## Compact JSON
Replace `serde_json::to_string_pretty()` with `serde_json::to_string()` across all MCP tool handlers and profiler export. AI consumers parse JSON programmatically — whitespace wastes tokens and serialization time.

## Redundant CDP elimination
After `goto()`, the handler currently calls:
1. `page.title()` — CDP Runtime.evaluate
2. `page.url()` — CDP Runtime.evaluate
3. `page.page_map()` — CDP Runtime.evaluate (contains title + URL)

The page_map JavaScript already extracts `document.title` and `window.location.href`. Remove calls 1 and 2, extract from page_map result.
