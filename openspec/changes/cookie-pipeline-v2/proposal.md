# Cookie Pipeline V2: Diagnostics, domain matching, and accurate reporting

## Problem

The cookie import pipeline has four interacting bugs:
1. `set_cookies()` returns `Result<(), RayoError>` — partial failures silently swallowed. MCP handler reports pre-injection count, not actual set count.
2. `import_cookies()` defaults to Chrome "Default" profile without reporting which profile was used or what alternatives exist.
3. Cookie decryption failures fall back to empty `raw.value`, then silently drop the cookie. No diagnostic output.
4. Domain matching uses SQL `LIKE '%domain%'` and Rust `.contains(domain)` — substring matching instead of RFC 6265 domain suffix matching.

## Solution

### `matches_domain()` — RFC 6265 domain matching
Single function in `cookie.rs` replacing all 5 substring call sites:
- `cookie_import.rs:287` — SQL LIKE query
- `tools/mod.rs:365` — cookie get filter
- `tools/mod.rs:379` — cookie clear filter
- `tools/mod.rs:406` — cookie save filter
- `browser.rs:392` — persist filter in auto-auth

Proper dot-boundary checking: `.example.com` matches `sub.example.com`, rejects `badexample.com`.

SQL LIKE replaced with fetch-all-then-filter in Rust. Filtering happens BEFORE decryption, so this is actually faster (fewer AES-128-CBC operations).

### `CookieImportResult` — diagnostic return from `import_cookies()`
```rust
pub struct CookieImportResult {
    pub cookies: Vec<SetCookie>,
    pub profile_used: String,
    pub found_in_db: usize,
    pub decrypt_failed: Vec<String>,
    pub empty_skipped: usize,
    pub available_profiles: Vec<String>,
}
```

### `CookieSetResult` — diagnostic return from `set_cookies()`
```rust
pub struct CookieSetResult {
    pub set: usize,
    pub failed: Vec<String>,
}
```

### MCP response composition
The import handler composes both results into an informative response:
- Success: `"Imported 5 cookie(s) from chrome (profile: 'Default') for domain 'company.inc'"`
- Partial: `" (2 rejected by Chrome: [\"__Host-sb-token\"])" `
- Decrypt: `" (3 failed decryption)"`
- Empty: `"No cookies found ... Other profiles available: [\"Profile 1\"]"`

## Scope

### rayo-core
- `cookie.rs` — add `CookieImportResult`, `CookieSetResult`, `matches_domain()` with tests
- `cookie_import.rs` — change `import_cookies()` return type, replace SQL LIKE with Rust filter, track decrypt failures (return `Err(name)` instead of falling back to empty)
- `browser.rs` — change `set_cookies()` from `Result<(), RayoError>` to `Result<CookieSetResult, RayoError>`, fix `.contains()` domain filter
- `lib.rs` — add re-exports

### rayo-mcp
- `tools/mod.rs` — update import/set/get/clear/save handlers for new return types and domain matching

## Boundary decisions
- `set_cookies()` return type change is backward-compatible for callers using `if let Err(e) = ...` (they discard Ok)
- `import_cookies()` return type is a breaking change — exactly 2 callers, both updated
- No auto-profile-retry — if Default has no cookies, report available profiles but don't silently try others
- Cookie names included in failure diagnostics only, not success responses

## Not in scope
- Auth detection changes (separate change: auth-detection-v2)
- Navigation reporting (separate change: auth-detection-v2)
- MCP sampling (separate change: mcp-sampling-auth)
