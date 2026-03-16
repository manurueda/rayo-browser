# Cache Coherence: Centralize invalidation, fix stale page_map after mutations

## Why
1. `type_text()` and `select_option()` don't invalidate page_map_cache — an AI agent calling page_map() after typing gets stale input values
2. Cache invalidation policy is scattered across click(), click_raw(), type_text(), execute_batch(), goto(), reload() with no central documentation
3. `resolve_selector()` acquires and releases the selector_cache mutex 3+ times per call
4. `type_text()` silently swallows clear-step errors with `.ok()`

## Solution
Extract a centralized `invalidate_after_mutation()` method. Call it from all mutation methods. Fix resolve_selector locking. Propagate clear errors as warnings.

## Scope

### rayo-core
- `browser.rs` — add `invalidate_after_mutation()` method
- `browser.rs` — call it from `click()`, `type_text()`, `select_option()`
- `browser.rs` — refactor `resolve_selector()` to hold single lock
- `browser.rs` — log warnings on failed clear evaluations in `type_text()`

## Not in scope
- CDP DOM event subscriptions for invalidation (future optimization)
- Extracting CacheManager as separate type (larger refactor)
