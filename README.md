# ⚡ **rayo-browser**

The fastest MCP browser tool. Rust. 95% fewer tokens. 17x faster workflows.

Now with **AI-native visual testing** — E2E test runner, visual regression, and a live dashboard. Zero SaaS fees.

## Why

Your AI agent is wasting its context window on screenshots. ⚡ rayo replaces screenshots with page maps, batches actions into single calls, and uses 9x less context than Playwright MCP. Same browser, radically less waste.

```
                              Speed                    Tokens
Wikipedia Research
  ⚡ rayo              136ms  ██░░░░░░░░░        5,597  █░░░░░░░░░░░░░░░░░░░
  Playwright          229ms  ███░░░░░░░░      100,367  ████████████████████
                               1.7x faster              94% fewer

Form Fill + Submit
  ⚡ rayo              190ms  ███░░░░░░░░        3,685  ███░░░░░░░░░░░░░░░░░
  Playwright          346ms  █████░░░░░░       23,311  ████████████████████
                               1.8x faster              84% fewer

HN Browse + Read
  ⚡ rayo              199ms  █░░░░░░░░░░        7,058  ██░░░░░░░░░░░░░░░░░░
  Playwright        1,274ms  ████████████      75,914  ████████████████████
                               6.4x faster              91% fewer
```

Real Claude Code workflows. Real websites. [Run them yourself →](#-reproduce-it)

## 📦 Get it

### Lite — Browser automation only

**Claude Code** — copy and paste this prompt:

```
Install rayo-browser: run `curl --proto '=https' --tlsv1.2 -LsSf https://github.com/manurueda/rayo-browser/releases/latest/download/rayo-mcp-installer.sh | sh` then run `claude mcp add rayo -- rayo-mcp`. Then add to my CLAUDE.md a "Browser automation" section that says: always use rayo MCP tools for browser automation instead of Playwright or Puppeteer, prefer rayo_observe with page_map mode over screenshots, use rayo_batch to combine 3+ sequential actions, write E2E tests as YAML in .rayo/tests/, and use rayo_visual for visual regression. Tell me to restart Claude Code.
```

That's it. One paste into Claude Code — it installs, configures MCP, updates your CLAUDE.md, and tells you to restart. Both `rayo-mcp` (MCP server) and `rayo-ui` (test runner + dashboard) are installed.

### Full — Browser automation + AI conductors

Install everything: browser automation, TDD fix pipelines, feature development, autonomous code cleanup, and architecture audits. All orchestrated through Claude Code slash commands.

**Claude Code** — copy and paste this prompt:

```
Install rayo-browser with conductors: First run `curl --proto '=https' --tlsv1.2 -LsSf https://github.com/manurueda/rayo-browser/releases/latest/download/rayo-mcp-installer.sh | sh` then `claude mcp add rayo -- rayo-mcp`. Then clone the rayo-browser repo to a temp dir: `git clone https://github.com/manurueda/rayo-browser.git /tmp/rayo-conductors && /tmp/rayo-conductors/conductors/install.sh && rm -rf /tmp/rayo-conductors`. Then add to my CLAUDE.md a "Browser automation" section that says: always use rayo MCP tools for browser automation instead of Playwright or Puppeteer, prefer rayo_observe with page_map mode over screenshots, use rayo_batch to combine 3+ sequential actions, write E2E tests as YAML in .rayo/tests/, use rayo_visual for visual regression. Also add a "Conductors" section that says: use /fix to launch TDD bug fix pipelines, /feature for end-to-end feature development, /guardian for autonomous cleanup, /architect for architecture audits, /cstatus for a unified dashboard. Tell me to restart Claude Code.
```

**What this installs:**
- `rayo-mcp` + `rayo-ui` — browser automation and visual testing
- `.fix/` — TDD fix conductor (diagnose → red → green → adversarial → E2E → merge)
- `.feature/` — feature conductor (spec → implement → test → break → UI test → merge)
- `.guardian/` — persistent cleanup agent (scans for violations, fixes them, runs forever)
- `.architect/` — architecture auditor (SQLite metrics DB → survey → audit → prescribe)
- Claude Code skills — `/fix`, `/feature`, `/guardian`, `/architect`, `/cstatus`, `/modular-delivery`, `/plan-ceo`

**Prerequisites:** [agent-deck](https://github.com/manurueda/agent-deck) (`npm i -g agent-deck`) for conductor session management.

**Cursor / Windsurf / any MCP client:**

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/manurueda/rayo-browser/releases/latest/download/rayo-mcp-installer.sh | sh
```

```json
{ "mcpServers": { "rayo": { "command": "rayo-mcp" } } }
```

> **Auto-updates:** ⚡ rayo checks for updates in the background and self-updates on next restart. Disable with `RAYO_NO_UPDATE=1`.

## 💡 The problem

Every MCP browser tool today works the same way: screenshot the page, send 5,500 tokens to the LLM, get one action back, repeat. Your agent burns context on pixels it can't even read properly.

| | Playwright MCP | ⚡ rayo |
|---|---|---|
| **See the page** | Screenshot: ~5,500 tokens | Page map: ~97 tokens |
| **Fill a form** | 8 tool calls, one field at a time | 1 batch call, all fields |
| **Tool definitions** | 22 tools → 13,200 tokens loaded | 8 tools → 1,800 tokens loaded |
| **Context cost** | 6.60% of 200k window | 0.90% of 200k window |

## 🧠 Page maps replace screenshots

This is how your agent sees a page. Not pixels. Structure.

```json
{
  "url": "https://httpbin.org/forms/post",
  "title": "httpbin.org",
  "interactive": [
    { "id": 0, "tag": "input", "type": "text", "name": "custname", "label": "Customer name",
      "bbox": { "x": 10, "y": 120, "width": 200, "height": 30 } },
    { "id": 1, "tag": "input", "type": "tel", "name": "custtel", "label": "Telephone" },
    { "id": 2, "tag": "input", "type": "email", "name": "custemail", "label": "E-mail" },
    { "id": 3, "tag": "button", "type": "submit", "text": "Submit" }
  ],
  "headings": ["HTML form"],
  "text_summary": "A form with customer name, phone, email, and submit button."
}
```

**97 tokens. The screenshot of this same page? 5,500 tokens. Same information, 57x cheaper.**

The agent reads the page map, knows there are 4 elements, and acts immediately. No vision model. No guessing coordinates. Every element now includes bounding box coordinates for visual testing.

## ⚡ Batch actions

Playwright MCP: 4 tool calls → 4 LLM round-trips → 4 screenshots back.

⚡ rayo: 1 call.

```json
{
  "tool": "rayo_batch",
  "actions": [
    { "action": "type", "id": 0, "value": "John Doe" },
    { "action": "type", "id": 1, "value": "555-0123" },
    { "action": "type", "id": 2, "value": "john@example.com" },
    { "action": "click", "id": 3 }
  ]
}
```

The agent already knows the IDs from the page map. It fills the form and submits in a single MCP call. No back-and-forth.

## 🧪 Visual Testing

⚡ rayo includes AI-native E2E visual testing — like Momentic, but faster and free.

### Just ask Claude

The easiest way to create tests is to ask your AI agent:

> "Add a test for the checkout flow"

Claude Code will:
1. Navigate to your app with `rayo_navigate`
2. Read the page structure with `rayo_observe` (page map)
3. Write a `.rayo/tests/checkout.test.yaml` based on what it sees
4. Run it with `rayo-ui run` to verify it passes

```
You: "add a test for the signup flow on localhost:3000"

Claude Code:
  → rayo_navigate { goto: "http://localhost:3000/signup" }
  → rayo_observe  { mode: "page_map" }
  ← sees: input[name=email], input[name=password], button "Sign Up"
  → writes .rayo/tests/signup.test.yaml
  → runs: rayo-ui run --suite "Signup Flow"
  → "✓ 3/3 steps passed (280ms)"
```

That's it. Claude already has all the rayo MCP tools — it can explore your app, understand the page, and write the test. No manual selectors, no recording, no setup.

Add this to your CLAUDE.md so Claude knows to use rayo for tests:

```markdown
## Testing

- Use rayo MCP tools to explore pages before writing tests
- Write E2E tests as YAML in `.rayo/tests/*.test.yaml`
- Use `rayo_observe` with `page_map` to discover selectors and element IDs
- Run tests with `rayo-ui run` after writing them
- Use `rayo_visual` to capture and compare screenshots for visual regression
```

### Auto-discover

Don't write tests by hand. Let rayo discover your app's flows automatically:

```bash
rayo-ui discover http://localhost:3000
```

rayo reads your source code to understand routes, forms, and API endpoints, then explores your app with the browser to verify what actually renders. It generates YAML test files for every discovered flow.

```
$ rayo-ui discover http://localhost:3000

  ⚡ Discovering flows...

  Framework: Next.js (app router)
  Routes from code: 23
  Routes explored: 23
  Flows detected: 15 (8 forms, 3 auth, 2 CRUD, 2 navigation)
  Console errors: 2

  Generated:
    .rayo/tests/login-form.test.yaml (4 steps)
    .rayo/tests/signup-form.test.yaml (5 steps)
    .rayo/tests/dashboard-navigation.test.yaml (3 steps)
    .rayo/tests/settings-form.test.yaml (6 steps)
    .rayo/tests/search-flow.test.yaml (3 steps)
    .rayo/tests/_smoke.test.yaml (23 steps)
    ... and 9 more

  Tests: 47/47 passing
  Health score: 87/100
  Report: .rayo/discover-report.md
```

**Source code + browser = complete coverage.** Code analysis gives intent (what the app should do). Page maps give reality (what actually renders). The delta is where bugs live.

Supported frameworks: Next.js, Express, Rails, Django, FastAPI, static HTML.

**Diff-aware mode for PRs:**

```bash
rayo-ui discover http://localhost:3000 --diff
```

Only discovers routes affected by your current branch. Perfect for CI — every PR gets auto-generated regression tests for exactly what changed.

### Test format

Tests are YAML files in `.rayo/tests/`:

```yaml
# .rayo/tests/login-flow.test.yaml
name: Login Flow
viewport: { width: 1280, height: 720 }

steps:
  - name: Navigate to login
    navigate: https://app.example.com/login
    assert:
      - page_map_contains: { text: "Sign in" }

  - name: Fill and submit
    batch:
      - { action: type, selector: "input[name='email']", value: "test@example.com" }
      - { action: type, selector: "input[name='password']", value: "secret" }
      - { action: click, selector: "button[type='submit']" }

  - name: Verify dashboard
    wait: { selector: ".dashboard", timeout_ms: 5000 }
    assert:
      - text_contains: Welcome back
      - screenshot:
          name: dashboard
          threshold: 0.02
```

### Assertions

| Type | What it checks |
|---|---|
| `page_map_contains` | Element exists by selector, text, role, or tag |
| `text_contains` | Visible page text includes a string |
| `screenshot` | Visual diff against baseline (auto-creates on first run) |
| `network_called` | API request was made matching a URL pattern |

### Visual diff engine

⚡ rayo's diff engine is Rust-native — no cloud service, no API calls, no per-screenshot fees.

```
Pipeline:  Hash pre-filter → YIQ pixel diff (AA detection) → SSIM scoring → Region clustering → Diff overlay
Speed:     <7ms for a 720p comparison
```

- **Anti-aliasing detection** — ignores subpixel rendering differences across environments
- **Perceptual scoring** — SSIM-based similarity (not just pixel counting)
- **Region clustering** — groups nearby changes into named regions with bounding boxes
- **Diff overlay** — generates a highlighted image showing exactly what changed
- **Animation freeze** — CSS injection disables animations for stable screenshots
- **Auto-baseline** — first run captures the baseline automatically

### MCP tool: `rayo_visual`

AI agents can also use visual testing directly via MCP — no YAML needed:

```json
{ "tool": "rayo_visual", "action": "capture", "name": "homepage" }
{ "tool": "rayo_visual", "action": "compare", "name": "homepage", "threshold": 0.01 }
{ "tool": "rayo_visual", "action": "baseline", "mode": "list" }
```

### CLI

```bash
rayo-ui discover http://localhost:3000  # Auto-discover and generate tests
rayo-ui discover http://localhost:3000 --diff  # Only changed routes
rayo-ui run                      # Run all suites
rayo-ui run --suite "Login Flow" # Run a specific suite
rayo-ui run --json report.json   # JSON report
rayo-ui run --html report.html   # Self-contained HTML report
rayo-ui list                     # List available test suites
rayo-ui ui                       # Open the dashboard
```

### Dashboard

```bash
rayo-ui ui
```

One command. Opens your browser to `http://localhost:4040` with the full dashboard — health stats, suite list, step-by-step results with diff reports, and a live runner with real-time WebSocket updates. Everything is embedded in the binary, no Node.js needed.

## 📊 Full benchmark breakdown

All numbers from real websites, 10 iterations, 3 warmup, warm browsers.

### How each tool sees a page

| Method | ⚡ rayo | Playwright | Puppeteer |
|--------|---------|------------|-----------|
| **page_map** | **<1ms · ~97 tokens** | — | — |
| text | <1ms · ~47 tokens | 1ms · ~33 tokens | 1ms · ~33 tokens |
| screenshot | 17ms · ~4,572 tokens | 17ms · ~5,526 tokens | 17ms · ~5,487 tokens |

Screenshots take the same 17ms everywhere — it's Chrome's rendering pipeline. The difference is that ⚡ rayo agents don't need them. Page maps give the same information in <1ms and 57x fewer tokens.

### Navigation speed

| Site | ⚡ rayo | Playwright | Puppeteer |
|------|---------|------------|-----------|
| example.com | 23ms | 18ms | 32ms |
| Wikipedia | 76ms | 72ms | 84ms |
| Hacker News | 75ms | 75ms | 89ms |

Navigation is network-bound. All three tools are the same speed — it's the same browser. The wins come from what happens *after* the page loads.

### Context window overhead

```
  ⚡ rayo           ~1,800 tokens  ██░░░░░░░░░░░░░░░░░░  0.90% of 200k
  Puppeteer MCP     ~4,500 tokens  ███████░░░░░░░░░░░░░  2.25% of 200k
  Playwright MCP   ~13,200 tokens  ████████████████████  6.60% of 200k
```

These are the tool definitions your agent loads before it does anything. Playwright MCP's 22 tool descriptions eat 6.6% of a 200k context window on arrival.

## 📈 Built-in profiler

Every ⚡ rayo operation is timed. Call `rayo_profile` to see where time goes:

```
RAYO PROFILE (3.6s total, benchmark suite)
  navigation: 3457ms (45%) | 47 ops | avg 74ms  | p95 173ms
  page_map:    170ms  (2%) | 77 ops | avg 2.2ms | p95 12ms
  screenshot:  244ms  (3%) | 13 ops | avg 19ms  | p95 25ms
  batch:        75ms  (1%) |  1 op  | avg 75ms
  dom.mutate:   75ms  (1%) |  4 ops | avg 19ms  | p95 23ms
SLOWEST: goto(wikipedia.org) 453ms
```

84% of time is network I/O (navigation). Page maps average 1.8ms. The tool itself is not the bottleneck — the internet is.

## 🔧 The 8 tools

| Tool | Does |
|------|------|
| `rayo_navigate` | goto (with `wait_until`), reload, back, forward, tabs |
| `rayo_observe` | page_map (with bounding boxes + element state), text, screenshot |
| `rayo_interact` | click, hover, type, press (Enter/Tab/Escape), select, scroll |
| `rayo_batch` | N actions in 1 call (with `abort_on_failure`) |
| `rayo_cookie` | set, get, clear, save, load, **import from Chrome/Arc/Brave/Edge** |
| `rayo_network` | capture, block, mock requests |
| `rayo_profile` | performance data (ai_summary, json, markdown, chrome_trace) |
| `rayo_visual` | **visual testing: capture baselines, compare screenshots, manage baselines** |

**Transparent auth:** Navigate to any authenticated page — ⚡ rayo auto-detects login walls, imports cookies from your real browser, and retries. Zero configuration.

## 🤖 AI Conductors

Conductors are autonomous multi-agent pipelines that orchestrate Claude Code sessions to perform complex software engineering tasks. Each conductor spawns specialized workers (in isolated git worktrees) and manages them through a defined protocol.

### `/fix` — TDD Bug Fix Pipeline

```bash
/fix "Brand system doesn't update when CEO pastes new palette"
```

Launches an autonomous pipeline:
1. **DIAGNOSE** — 3 parallel workers investigate (top-down, bottom-up, git history) + challenger validates
2. **RED** — write failing tests that prove the bug exists
3. **GREEN** — implement the minimum fix to make tests pass
4. **ADVERSARIAL** — breaker writes 20-50 edge cases to harden the fix
5. **E2E** — browser verification with rayo (for UI bugs)
6. **MERGE** — tsc + vitest + lint, then merge to main

Multiple fixes can run in parallel on independent bugs.

### `/feature` — Feature Development Pipeline

```bash
/feature "Add dark mode toggle to settings page"
```

Full end-to-end feature delivery:
1. **SPEC** — modular-delivery artifacts (proposal, design, tasks, verification scenarios)
2. **IMPLEMENT** — one worker per module, sequential
3. **TEST** — 100% coverage for every new function
4. **BREAK** — adversarial testing loop until zero bugs
5. **SCENARIO** — real-world verification against live services
6. **UI TEST** — browser verification with rayo
7. **MERGE** — double review pass, then merge and push

### `/guardian` — Persistent Cleanup Agent

```bash
/guardian
```

Runs forever. Three worker types:
- **SCANNER** — finds SRP/DRY/DI violations, test gaps, god components
- **CLEANER** — fixes one task at a time in isolated worktrees
- **BUG HUNTER** — writes adversarial tests to find bugs (runs in parallel with cleaners)

Auto-merges to main, auto-pushes. Rescans every 6 tasks. Never stops.

### `/architect` — Strategic Codebase Audit

```bash
/architect
```

Read-only audit pipeline:
1. **COLLECT** — TypeScript compiler API + dependency-cruiser + knip + git history → SQLite DB
2. **SURVEY** — queries DB, produces structured census
3. **AUDIT** — diagnoses architectural smells, scores 9 dimensions
4. **PRESCRIBE** — concrete transformation proposals with execution order

Hands off approved proposals to the Guardian's work queue.

### `/cstatus` — Unified Dashboard

```bash
/cstatus
```

Shows all running conductors in one table: fix, feature, guardian, architect.

### How It Works

Conductors use [agent-deck](https://github.com/manurueda/agent-deck) for session management. Each conductor:
- Spawns worker agents in isolated **git worktrees** (no conflicts)
- Monitors workers by polling every 60 seconds
- Auto-responds to worker questions (keeps them moving)
- Merges, validates, and cleans up after each phase
- Tracks state in JSON files for monitoring

```
User → /fix "bug"
  → launch.sh creates worktree + agent-deck session
    → Conductor reads conductor-claude.md (its protocol)
      → Spawns workers from *-prompt.md templates
        → Workers run in isolated worktrees
          → Conductor merges results, iterates until clean
```

### Manual Installation

If you prefer to install conductors manually:

```bash
git clone https://github.com/manurueda/rayo-browser.git /tmp/rayo-install
/tmp/rayo-install/conductors/install.sh
rm -rf /tmp/rayo-install
```

Or `conductors/install.sh --dry` to preview what it would do.

## 🏗️ Architecture

```
AI Agent → MCP (stdio) → rayo-mcp → rayo-core → chromiumoxide → Chrome
                           │   │         │
                           │   │      rayo-profiler
                        rayo-rules      │
                        rayo-updater rayo-visual

rayo-ui CLI/UI → rayo-core + rayo-visual → Chrome
```

7 Rust crates:

| Crate | Purpose | Dependencies |
|---|---|---|
| `rayo-profiler` | Built-in profiling (14 span categories) | None |
| `rayo-updater` | Self-update via cargo-dist + axoupdater | None |
| `rayo-visual` | Image diff engine (YIQ, SSIM, clustering, baselines) | None |
| `rayo-core` | Browser automation (page maps, batch, cache, tabs, network) | profiler, visual |
| `rayo-rules` | Speed rules engine | None |
| `rayo-ui` | E2E test runner (YAML, assertions, reports, web server) | core, visual, profiler |
| `rayo-mcp` | MCP server binary (8 tools) | core, rules, updater, profiler |

## 🧪 Reproduce it

```bash
# Run the full competitor benchmark (vs Playwright + Puppeteer)
cd bench/competitors && npm install && npx tsx src/run-benchmarks.ts

# Run internal micro-benchmarks
cargo bench
```

## 🛠️ Development

```bash
cargo build --workspace          # Build all
cargo test --workspace           # Test (needs Chrome)
cargo bench                      # Benchmark
cargo run --bin rayo-mcp         # Start MCP server
cargo run --bin rayo-ui -- run # Run E2E test suites
cargo run --bin rayo-ui -- ui  # Open dashboard (embedded UI)
cargo clippy --workspace         # Lint
cargo fmt --check --all          # Check formatting
```

## License

MIT
