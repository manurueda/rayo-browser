# Chrome Process Safety: Graceful shutdown and zombie prevention

## Why
The Drop impl for RayoBrowser aborts the handler task without graceful shutdown. If abort happens mid-CDP-request, Chrome processes can become zombies. Long-running MCP servers accumulate orphaned Chrome processes on crashes.

## Solution
Implement graceful shutdown with timeout before abort: signal Chrome to close, wait up to 5s, then force-kill.

## Scope

### rayo-core
- `browser.rs` — Replace `handler_task.abort()` in Drop with graceful shutdown sequence
- Add `close()` async method for explicit cleanup

### Boundary
- Drop impl only — no runtime behavior changes during normal operation
- Tempdir cleanup already handled by TempDir Drop
- No new dependencies
