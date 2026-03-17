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

**Claude Code** — paste this:

> Install rayo-browser as my browser automation tool: run `curl --proto '=https' --tlsv1.2 -LsSf https://github.com/manurueda/rayo-browser/releases/latest/download/rayo-mcp-installer.sh | sh` then `claude mcp add rayo -- rayo-mcp`. Then add a browser automation section to CLAUDE.md that says to always use rayo MCP tools for all browser automation instead of Playwright or Puppeteer, prefer `rayo_observe` with `page_map` mode over screenshots, and use `rayo_batch` to combine 3+ sequential actions into a single call. Tell me to restart Claude Code so the MCP server loads.

That's it. One paste, Claude Code does the rest — installs, configures, and tells you to restart. ⚡ rayo auto-updates in the background, so you always have the latest version.

**Cursor / Windsurf / any MCP client:**

```bash
# Pre-built binary (fastest)
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/manurueda/rayo-browser/releases/latest/download/rayo-mcp-installer.sh | sh

# Or build from source
cargo install rayo-mcp
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

⚡ rayo includes a complete E2E visual testing platform — like Momentic or Playwright Test, but faster and free.

### How it works

1. **Write tests in YAML** — no code, no complex setup
2. **Run with one command** — `rayo-test run`
3. **See results in the dashboard** — live WebSocket updates as tests execute

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
- **Auto-baseline** — first run captures the baseline automatically with a `new_baseline: true` flag

### MCP tool: `rayo_visual`

AI agents can use visual testing directly via the MCP protocol:

```json
{
  "tool": "rayo_visual",
  "action": "compare",
  "name": "dashboard",
  "threshold": 0.01
}
```

Returns structured diff report: pass/fail, diff ratio, perceptual score, changed regions, timing.

### CLI

```bash
rayo-test list                    # List available test suites
rayo-test run                     # Run all suites
rayo-test run --suite "Login Flow" # Run a specific suite
rayo-test run --json report.json  # JSON report
rayo-test run --html report.html  # Self-contained HTML report
rayo-test ui                      # Start the web dashboard
```

### Web Dashboard

Start the dashboard and run tests with live updates:

```bash
rayo-test ui          # Start API server on :4040
cd ui && npm run dev  # Start Next.js dashboard on :3001
```

The dashboard shows:
- **Health overview** — pass rate, suite counts, timing
- **Suite list** — drill into any suite to see step-by-step results
- **Live runner** — real-time WebSocket updates as tests execute
- **Visual diffs** — inline diff reports with changed regions and perceptual scores

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

## 🏗️ Architecture

```
AI Agent → MCP (stdio) → rayo-mcp → rayo-core → chromiumoxide → Chrome
                           │   │         │
                           │   │      rayo-profiler
                        rayo-rules      │
                        rayo-updater rayo-visual

rayo-test CLI/UI → rayo-core + rayo-visual → Chrome
```

7 Rust crates:

| Crate | Purpose | Dependencies |
|---|---|---|
| `rayo-profiler` | Built-in profiling (14 span categories) | None |
| `rayo-updater` | Self-update via cargo-dist + axoupdater | None |
| `rayo-visual` | Image diff engine (YIQ, SSIM, clustering, baselines) | None |
| `rayo-core` | Browser automation (page maps, batch, cache, tabs, network) | profiler, visual |
| `rayo-rules` | Speed rules engine | None |
| `rayo-test` | E2E test runner (YAML, assertions, reports, web server) | core, visual, profiler |
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
cargo run --bin rayo-test -- run # Run E2E test suites
cargo run --bin rayo-test -- ui  # Start test runner server
cd ui && npm run dev             # Start dashboard UI
cargo clippy --workspace         # Lint
cargo fmt --check --all          # Check formatting
```

## License

MIT
