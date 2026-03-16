# Design: Chrome Launch Performance

## Flags to add
These are standard automation flags used by Playwright and Puppeteer:
- `--disable-extensions` — no extension loading
- `--disable-background-networking` — no background network requests
- `--disable-sync` — no Chrome sync
- `--metrics-recording-only` — metrics collected but not reported
- `--no-first-run` — skip first-run dialogs
- `--disable-background-timer-throttling` — prevent timer throttling in background tabs
- `--disable-default-apps` — no default app installation

## Placement
Added in `launch()` method alongside existing `--disable-gpu` and `--disable-dev-shm-usage` flags.
