# Auth Detection V2: Confidence scoring, OAuth detection, and redirect reporting

## Problem

Two bugs in auth wall detection and navigation reporting:
1. `is_auth_redirect()` only fires when the final URL contains auth keywords. Apps that redirect to a homepage or use OAuth-only login bypass detection entirely.
2. `is_login_page()` only checks for password input fields. Google OAuth pages have buttons, not password fields.
3. Navigation response says "Navigated to {final_url}" with no indication that the requested URL was different. The agent must string-compare to notice a redirect.
4. Auto-auth retry doesn't verify whether auth actually succeeded.

## Solution

### `detect_auth_wall()` — confidence-scored auth detection
Replaces two narrow boolean checks with weighted signal analysis:

| Signal | Weight | Catches |
|--------|--------|---------|
| Auth keyword in URL | 0.8 | Existing behavior |
| Password input | 0.9 | Existing behavior |
| OAuth provider links in hrefs | 0.7 | Google OAuth, GitHub OAuth, etc. |
| OAuth text in buttons/links | 0.6 | "Sign in with Google", "Continue with" |
| Suspicious path change | 0.3 | Homepage redirects (/play -> /) |
| Auth headings | 0.3 | "Sign In", "Welcome back" |
| Cross-domain redirect | 0.2 | SSO redirects |
| Auth title | 0.2 | Weak corroboration |

Threshold: 0.5. Weights hardcoded — implementation detail, not user config.

### `is_meaningful_redirect()` — trivial redirect filtering
String-based URL normalization (no `url` crate):
- Strips trailing slashes, `www.`, scheme differences, fragments
- `/play` -> `/` is meaningful. `http://` -> `https://` is not.

### `NavigationResult` — wraps `PageMap` with navigation metadata
```rust
pub struct NavigationResult {
    pub map: PageMap,
    pub requested_url: String,
    pub redirected: bool,
    pub auto_auth: AutoAuthStatus, // NotNeeded | Succeeded | Failed
}
```

### MCP response annotations
- Normal: `"Navigated to https://example.com/page"`
- Redirect: `"\n⚠ Redirected from https://example.com/play"`
- Auth failed: `"\n⚠ Auto-auth: auth wall detected but cookie import failed"`
- No clutter on happy path.

## Scope

### rayo-core
- `auth.rs` — add `AuthDetection`, `AuthSignal`, `detect_auth_wall()`, OAuth/heading detection helpers, `is_meaningful_redirect()`, `normalize_url()`. Keep `is_auth_redirect()` and `is_login_page()` for backward compat.
- `browser.rs` — add `NavigationResult`, `AutoAuthStatus`. Change `goto_with_auto_auth()` to return `NavigationResult`. Add `LlmAuthChecker` type alias for MCP sampling bridge. Add retry verification.
- `lib.rs` — add re-exports

### rayo-mcp
- `tools/mod.rs` — update navigate handler to consume `NavigationResult`, build annotated status lines

## Boundary decisions
- `PageMap` unchanged — navigation metadata doesn't belong in the page representation
- Batch executor uses `goto_raw`, not `goto_with_auto_auth` — unaffected
- `LlmAuthChecker` type alias defined but unused until mcp-sampling-auth change lands
- Confidence weights hardcoded, not exposed through `.rayo-rules`
- `goto_with_auto_auth` gains `llm_checker: Option<&LlmAuthChecker>` parameter (defaults to None)

## Not in scope
- Cookie pipeline changes (separate change: cookie-pipeline-v2)
- Actual MCP sampling implementation (separate change: mcp-sampling-auth)
- Localized login page detection (English-only for now, OAuth links are language-independent)
