# Profiler Safety: Replace .unwrap() with graceful mutex handling

## Why
The profiler uses `std::sync::Mutex` and calls `.lock().unwrap()` at 5 call sites. If any thread panics while holding the profiler lock, the mutex becomes poisoned and ALL subsequent profiler operations panic, crashing the MCP server. The Drop impl already handles this gracefully — public methods should too.

## Solution
Replace `.lock().unwrap()` with graceful error handling that returns early or logs a warning on poisoned mutex, matching the pattern already used in the SpanGuard Drop impl.

## Scope

### rayo-profiler
- `lib.rs` — replace 5 `.lock().unwrap()` calls with `.lock().ok()` or `match` pattern
- Affected methods: `start_span()` (2 sites), `spans()`, `reset()`, `span_count()`

## Not in scope
- Switching to parking_lot::Mutex (would also fix this, but is a dependency change)
- Lock-free ring buffer (larger architectural change)
