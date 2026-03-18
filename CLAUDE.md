# rayo-browser

The fastest MCP browser automation tool. Rust-powered, benchmark-proven, AI-native.

## Architecture

```
AI Agent → MCP (stdio) → rayo-mcp → rayo-core → chromiumoxide → Chrome
                           │   │         │
                           │   │      rayo-profiler
                        rayo-rules      │
                        rayo-updater rayo-visual

rayo-ui CLI/UI → rayo-core + rayo-visual → Chrome
```

7 crates:
- `rayo-profiler` — profiling (no deps on other rayo crates)
- `rayo-updater` — self-update via cargo-dist/axoupdater (no deps on other rayo crates)
- `rayo-visual` — image diff engine: YIQ pixel diff, SSIM, clustering, baselines (no deps on other rayo crates)
- `rayo-core` — browser intelligence (page maps, batch, cache, waits, tabs, network, visual extensions)
- `rayo-rules` — speed rules engine
- `rayo-ui` — E2E test runner: YAML tests, assertions, reports, web server
- `rayo-mcp` — MCP server binary (7 tools)

## Commands

```bash
cargo build --workspace          # Build all
cargo test --workspace           # Run all tests (needs Chrome)
cargo bench                      # Run criterion benchmarks
cargo run --bin rayo-mcp         # Start MCP server
cargo run --bin rayo-ui -- run # Run E2E test suites
cargo run --bin rayo-ui -- ui  # Start test runner web UI
cargo clippy --workspace         # Lint
cargo fmt --check --all          # Check formatting
```

## Integration tests require Chrome

Tests that need a browser will auto-skip if Chrome isn't found.
Install Chrome or Chromium to run integration tests.
Tests use a local axum server serving fixtures from `tests/fixtures/`.

## 7 MCP Tools

| Tool | Purpose | Tokens |
|------|---------|--------|
| `rayo_navigate` | goto, reload, back, forward, new_tab, close_tab, list_tabs, switch_tab | ~300 |
| `rayo_observe` | page_map, text, screenshot | ~300 |
| `rayo_interact` | click, type, select, scroll | ~250 |
| `rayo_batch` | execute multiple actions in 1 call | ~350 |
| `rayo_cookie` | set, get, clear cookies | ~250 |
| `rayo_network` | capture, block, mock, requests | ~250 |
| `rayo_profile` | get profiling results | ~150 |

## Installation

```
cargo install --git https://github.com/manurueda/rayo-browser rayo-mcp
```

Then: `claude mcp add rayo -- rayo-mcp`

Restart Claude Code so the MCP server loads. Then add to your CLAUDE.md:

```
## Browser automation

- Always use rayo MCP tools (`rayo_*`) for all browser automation — never Playwright or Puppeteer
- Prefer `rayo_observe` with `page_map` mode over screenshots for page understanding
- Use `rayo_batch` to combine 3+ sequential actions into a single call
```

## Key conventions

- chromiumoxide handles CDP — do not rewrite CDP protocol handling
- Page maps are the primary page representation (~500 tokens vs 100k for screenshots)
- Batch execution is the primary speed optimization for AI agents
- Profiling is on by default
- All browser integration tests share one browser instance (avoid Chrome process conflicts)
- Multi-tab: TabManager in rayo-core, tab_id param on tools
- Network interception: NetworkInterceptor in rayo-core, rayo_network tool in rayo-mcp
- Chrome sandbox auto-detection: only disabled in CI/containers (CI env var, /.dockerenv)
- Each browser instance gets a unique tempdir (no shared profile conflicts)
- Auto-update: rayo-updater checks GitHub Releases on startup (background), replaces binary via axoupdater
- Auto-update state stored in ~/.rayo/ (last-check, update-marker, lock)
- Disable auto-update: RAYO_NO_UPDATE=1
- Release pipeline: cargo-dist builds platform binaries on git tag push

## Visual testing (rayo-ui)
- Test definitions in `.rayo/tests/*.test.yaml`
- Baselines stored in `.rayo/baselines/` as PNG + metadata JSON
- rayo-visual is a pure image crate — zero rayo deps, publishable independently
- Diff pipeline: hash pre-filter → YIQ pixel diff (AA detection) → SSIM → clustering → overlay
- Page maps include bounding boxes (getBoundingClientRect) for visual testing
- Animation freeze via CSS injection before screenshot capture
- PNG for visual testing, JPEG for regular screenshots
- Viewport configurable via ViewportConfig (default 1280x720)
- rayo-ui web server: REST API + WebSocket for live test updates
- `rayo-ui discover <url>` auto-generates tests from source code + browser exploration
- Framework analyzers: Next.js, Express, Rails, Django, FastAPI, static HTML
- Diff-aware mode: `--diff` flag scopes discovery to changed routes on current branch
- Flow detection: forms, auth, CRUD, navigation, search — multi-step test sequences
- Discovery report with health score written to .rayo/discover-report.md
