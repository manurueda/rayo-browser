# Observation Layer: Fix silent failures, complete network capture

## Why
1. `text_content()` uses `querySelector` (returns first match only) — agents silently get incomplete data
2. `text_content()` crashes with `TypeError: null.textContent` when selector doesn't match
3. `page_map()` ignores the `selector` parameter — always returns full page regardless
4. `rayo_network capture` + `requests` always returns 0 results — CDP Fetch events are not wired
5. `execute_batch()` has no way to abort on failure — actions run against destroyed contexts after navigation

## What Changes

### rayo-core
- `browser.rs` — `text_content()`: use `querySelectorAll`, join with `\n`, null-safe, new `max_elements` param (default 50)
- `browser.rs` — `page_map()`: accept optional `selector` param, scope interactive elements/headings/text_summary to subtree
- `browser.rs` — `execute_batch()`: new `abort_on_failure` param, skip remaining actions on failure
- `browser.rs` — `enable_network_interception()`: wire CDP `Fetch.enable()` + `Fetch.requestPaused` event handler
- `browser.rs` — `press_key()`: new method for CDP key dispatch
- `batch.rs` — add `Press` variant to `BatchAction`
- `page_map.rs` — enhanced text_summary extraction (scope to main/article, filter invisible elements)

### rayo-mcp
- `tools/mod.rs` — pass `selector`, `max_elements` to observe handlers; extract `abort_on_failure` for batch
- `server.rs` — update tool schemas (max_elements, abort_on_failure); wire network interception on page creation

## Not in scope
- CDP DOM mutation events for cache invalidation (see `cache-coherence` change)
- Selector cache wiring into `resolve_selector()` (see `smart-cache` change)
- Event-driven waits replacing polling (see `event-driven-waits` change)
