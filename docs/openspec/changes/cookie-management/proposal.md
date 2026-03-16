# Cookie Management: Authenticated page access for rayo-browser

## Problem
rayo-browser (headless) cannot access authenticated pages. Navigating to protected routes redirects to login. No mechanism exists to set, read, or clear cookies via MCP.

## Solution
New `rayo_cookie` MCP tool (tool #6) backed by cookie methods on `RayoPage`. Uses CDP cookie APIs via chromiumoxide, exposed through rayo-core owned types.

## Scope

### rayo-core
- `cookie.rs` — owned types (`SetCookie`, `CookieInfo`, `SameSite`) that decouple downstream crates from chromiumoxide
- `browser.rs` — `set_cookies()`, `get_cookies()`, `delete_cookie()`, `clear_cookies()` on `RayoPage`, with internal conversion to/from CDP types
- `clear_cookies()` uses `ClearBrowserCookiesParams` (single CDP call, not N+1)

### rayo-mcp
- `tools/cookie.rs` — `handle_cookie()` handler with `set`, `get`, `clear` actions
- `server.rs` — tool definition + route

### Integration test
- Cookie set → get → clear → verify cycle in shared browser test

## Boundary decisions
- chromiumoxide types (`CookieParam`, `Cookie`, `TimeSinceEpoch`, `CookieSameSite`) stay inside rayo-core — not re-exported
- rayo-mcp depends only on `SetCookie`, `CookieInfo`, `SameSite` from rayo-core
- Browser cookie database import (Chrome/Arc SQLite) is out of scope — separate capability slice

## Not in scope
- Cookie import from real browser databases (requires OS keychain decryption)
- Cookie persistence across rayo sessions
