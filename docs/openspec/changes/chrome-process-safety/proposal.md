# Chrome Process Safety: Graceful shutdown and zombie prevention

## Why
The Drop impl for RayoBrowser aborts the handler task without graceful shutdown. If abort happens mid-CDP-request, Chrome processes can become zombies. Long-running MCP servers accumulate orphaned Chrome processes on crashes.

## Solution
Add an explicit async `close()` method that sends a CDP close command, waits up to 5 seconds for the Chrome process to exit, then force-kills if needed. The MCP server calls `close()` after the session ends. The Drop impl remains as a safety net for cases where `close()` is not called.

## Scope

### rayo-core
- `browser.rs` — Add `close()` async method for graceful Chrome shutdown
- `browser.rs` — Drop impl remains as safety net (abort handler task)

### rayo-mcp
- `server.rs` — Call `browser.close()` after MCP session ends

## Not in scope
- Chrome process monitoring/watchdog
- Automatic restart on Chrome crash
