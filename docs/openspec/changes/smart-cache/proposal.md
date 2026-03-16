# Smart Cache: Selective invalidation + selector cache wiring

## Why
1. The SelectorCache is built but never used — selectors are resolved via page_map every time
2. Every click/type unconditionally invalidates the entire selector cache AND page_map cache
3. Most interactions don't structurally change the DOM

## Solution
Wire the selector cache into resolve_selector(). Stop unconditionally invalidating caches on every interaction — only invalidate page_map on navigation.

## Scope

### rayo-core
- `browser.rs` — `resolve_selector()`: check selector cache before page_map fallback
- `browser.rs` — `click()`: keep selector cache invalidation but stop clearing page_map cache
- `browser.rs` — `type_text()`: no cache invalidation needed (typing doesn't change DOM structure)
- `selector_cache.rs` — export hit/miss stats

## Not in scope
- CDP DOM event subscriptions for invalidation (future optimization)
- remote_object_id caching (requires deeper chromiumoxide integration)
