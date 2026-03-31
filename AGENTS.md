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
- `rayo-mcp` — MCP server binary (9 tools)

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

## Installation

```
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/manurueda/rayo-browser/releases/latest/download/rayo-mcp-installer.sh | sh
```

Then: `Codex mcp add rayo -- rayo-mcp`

Restart Codex so the MCP server loads. Then add to your AGENTS.md:

```
## Browser automation

- Always use rayo MCP tools (`rayo_*`) for all browser automation — never Playwright or Puppeteer
- Prefer `rayo_observe` with `page_map` mode over screenshots for page understanding
- Use `rayo_batch` to combine 3+ sequential actions into a single call
```

## 9 MCP Tools

| Tool | Purpose | Tokens |
|------|---------|--------|
| `rayo_navigate` | goto, reload, back, forward, new_tab, close_tab, list_tabs, switch_tab | ~300 |
| `rayo_observe` | page_map, text, screenshot, inspect (computed CSS, applied rules, diagnostics) | ~300-530 |
| `rayo_interact` | click, type, select, scroll | ~250 |
| `rayo_batch` | execute multiple actions in 1 call | ~350 |
| `rayo_cookie` | set, get, clear cookies | ~250 |
| `rayo_network` | capture, block, mock, requests | ~250 |
| `rayo_profile` | get profiling results | ~150 |
| `rayo_visual` | screenshot comparison, baseline management | ~200 |
| `rayo_report` | get collected errors for bug reporting | ~100 |

## Error reporting (auto-issue filing)

When any `rayo_*` MCP tool fails:
1. The error is automatically collected in-memory by rayo's ErrorCollector
2. A Codex hook logs it to `~/.rayo/error-log.jsonl` and notifies you
3. If a tool fails **3+ times in a session**, you MUST:
   a. Call `rayo_report` with `action: "get"` to retrieve structured error data
   b. File a GitHub issue: `gh issue create --repo manurueda/rayo-browser --title "Bug: <tool> — <short description>" --body "<rayo_report output + what you were trying to do>"`
   c. Label it `bug`
   d. Tell the user you filed the issue, then continue with a fallback approach
4. For a single failure: retry once, then fall back to an alternative approach

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
- Element inspection: `rayo_observe mode=inspect` for computed CSS, applied rules, box model, visibility diagnosis
- When verifying CSS changes, use `inspect` mode instead of screenshots — more reliable and token-efficient
- Inspect uses CDP CSS domain (CSS.getComputedStyleForNode, CSS.getMatchedStylesForNode) + JS diagnostics
- Inspect supports property filtering (curated ~40 default, category shorthands, `all: true`)
- Style diff via `diff: true` — compares against cached previous inspect (separate cache, survives DOM mutations)
- Expect assertions via `expect: {"background-color": "#hex"}` — returns pass/fail per property
- First use of `tokio::try_join!` for parallel CDP calls in browser.rs

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
