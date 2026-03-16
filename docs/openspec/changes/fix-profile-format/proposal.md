# Fix Profile Format: Wire format parameter to export methods

## Why
The rayo_profile tool definition declares a `format` enum (ai_summary, json, markdown, chrome_trace) but the handler ignores it and always returns ai_summary. The Profiler has all four export methods implemented but unreachable via MCP.

## Solution
Parse the `format` parameter in handle_profile() and dispatch to the correct Profiler export method.

## Scope

### rayo-mcp
- `tools/mod.rs` — `handle_profile()`: accept params, match on format, call appropriate export
- Handler must accept `&serde_json::Map<String, Value>` for params (currently only takes `&Arc<Profiler>`)

## Not in scope
- New export formats
- Profile format schema changes
