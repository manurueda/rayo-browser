# Design: Smart Cache

## Selector cache wiring
In `resolve_selector()`, when resolving by ID:
1. Check page_map_cache first (already done)
2. If cache miss, generate new page_map (already done)
3. After finding selector from page_map, store it in selector_cache for future CSS-based lookups

## Smarter invalidation
- `click()`: Keep `selector_cache.invalidate()` (elements may move). Remove `page_map_cache = None` — the page_map is still structurally valid after a click (elements are still there, just state may change).
- `type_text()`: No invalidation needed — typing into an input doesn't change DOM structure.
- `goto()` / `reload()`: Full invalidation of both caches (navigation changes everything).
