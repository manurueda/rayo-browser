# Design: Batch Pipelining

## Approach
Add an `invalidate_cache` flag to internal interaction methods. When called from batch, pass `false`. The batch executor does a single invalidation after all actions complete.

Alternative: Instead of flag-based approach, refactor click/type to not touch caches, and have the caller (batch or individual tool handler) decide when to invalidate. But this changes the public API contract — safer to use internal methods.

Chosen approach: Add private `click_inner()` and `type_text_inner()` that take a `skip_invalidation: bool` parameter. Public methods call inner with `false`. Batch calls inner with `true` and invalidates once at the end.
