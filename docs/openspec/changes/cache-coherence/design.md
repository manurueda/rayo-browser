# Design: Cache Coherence

## Centralized invalidation
```rust
/// Invalidate caches after a DOM mutation (click, type, select).
/// Selector cache uses generation-based invalidation.
/// Page map cache is cleared entirely since DOM structure may have changed.
async fn invalidate_after_mutation(&self) {
    self.selector_cache.lock().await.invalidate();
    *self.page_map_cache.lock().await = None;
}
```

## Usage
- `click()` → `click_raw()` + `invalidate_after_mutation()`
- `type_text()` → interaction + `invalidate_after_mutation()`
- `select_option()` → interaction + `invalidate_after_mutation()`
- `click_raw()` — NO invalidation (batch defers to end)
- `execute_batch()` → `invalidate_after_mutation()` at end (already done for selector, add page_map)
- `goto()` / `reload()` → keep existing inline invalidation (same effect)

## resolve_selector lock consolidation
Hold one lock for the entire cache lookup path instead of 3 separate lock/unlock cycles.

## type_text clear error handling
Replace `.ok()` with `tracing::warn!` on failure — callers should know if clear failed.
