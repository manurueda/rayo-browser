# Chrome Launch Performance: Optimized launch flags

## Why
Chrome launches with minimal flags, leaving background services (extensions, sync, networking, translation) active. These consume CPU during the critical first navigation, adding 50-200ms overhead.

## Solution
Add performance-oriented Chrome flags that disable unnecessary background services, matching Playwright's launch configuration.

## Scope

### rayo-core
- `browser.rs` — `launch()` method
  - Add: `--disable-extensions`, `--disable-background-networking`, `--disable-sync`, `--metrics-recording-only`, `--no-first-run`, `--disable-background-timer-throttling`, `--disable-default-apps`

### Boundary
- Launch args only — no runtime behavior changes
- Existing sandbox auto-detection (CI/container) preserved
- No new dependencies

## Not in scope
- Chrome auto-download/management
- Custom Chrome binary path configuration
