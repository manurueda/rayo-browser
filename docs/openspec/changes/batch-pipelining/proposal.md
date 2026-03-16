# Batch Pipelining: Skip per-action cache invalidation in batch

## Why
Each action in a batch independently invalidates caches. A batch of 5 clicks does 5 selector cache invalidations and 5 page_map cache clears. This is wasteful — only one invalidation at the end of the batch is needed.

## Solution
Defer cache invalidation to batch completion. Individual actions within a batch skip cache invalidation; the batch executor does a single invalidation pass at the end.

## Scope

### rayo-core
- `browser.rs` — add internal `click_no_invalidate()` / interaction methods that skip cache ops
- `browser.rs` — `execute_batch()`: use no-invalidate variants, single invalidation at end

## Not in scope
- CDP command coalescing (combining multiple Input events into one CDP call)
- Parallel action execution (actions still run sequentially within batch)
