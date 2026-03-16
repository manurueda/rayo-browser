# Response Efficiency: Compact JSON + redundant CDP elimination

## Why
All MCP responses use `serde_json::to_string_pretty()` adding ~20-40% unnecessary whitespace. After `goto`, the handler makes 3 separate CDP calls (title, URL, page_map) when page_map already contains title and URL.

## Solution
Switch to compact JSON serialization. Remove redundant `page.title()` and `page.url()` CDP calls after navigation, using page_map data instead.

## Scope

### rayo-mcp
- `tools/*.rs` — replace all `to_string_pretty()` with `to_string()`
- `tools/mod.rs` — `handle_navigate` goto branch: remove separate title/URL calls, derive from page_map

### rayo-profiler
- `export.rs` — switch `to_json()` from pretty to compact

### Boundary
- Serialization format only — no behavioral changes
- AI consumers get identical data, just compact
- Navigation response derived from page_map instead of 3 separate CDP calls

## Not in scope
- Streaming MCP responses
- Delta encoding between responses
