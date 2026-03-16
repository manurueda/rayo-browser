# Fix Wait and Clear: Null-safe MutationObserver + replace deprecated execCommand

## Why
1. `wait_for_selector()` assumes `document.body` exists. During early page load, body is null and `observer.observe(null, ...)` throws a TypeError, crashing the wait.
2. `type_text()` clear logic uses `document.execCommand('delete')` which is deprecated and will be removed from Chrome.

## Solution
1. Fall back to `document.documentElement` when `document.body` is null.
2. Replace execCommand with direct value clearing + input/change event dispatch.

## Scope

### rayo-core
- `browser.rs` — `wait_for_selector()`: null-safe observer target
- `browser.rs` — `type_text()`: replace execCommand with value assignment + events

## Not in scope
- iframe support for MutationObserver
- ContentEditable clearing (only input/textarea)
