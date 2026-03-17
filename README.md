# ⚡ **rayo-browser**

The fastest MCP browser tool. Rust. 95% fewer tokens. 17x faster workflows.

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
| **Tool definitions** | 22 tools → 13,200 tokens loaded | 7 tools → 1,500 tokens loaded |
| **Context cost** | 6.60% of 200k window | 0.75% of 200k window |

## 🧠 Page maps replace screenshots

This is how your agent sees a page. Not pixels. Structure.

```json
{
  "url": "https://httpbin.org/forms/post",
  "title": "httpbin.org",
  "interactive": [
    { "id": 0, "tag": "input", "type": "text", "name": "custname", "label": "Customer name" },
    { "id": 1, "tag": "input", "type": "tel", "name": "custtel", "label": "Telephone" },
    { "id": 2, "tag": "input", "type": "email", "name": "custemail", "label": "E-mail" },
    { "id": 3, "tag": "button", "type": "submit", "text": "Submit" }
  ],
  "headings": ["HTML form"],
  "text_summary": "A form with customer name, phone, email, and submit button."
}
```

**97 tokens. The screenshot of this same page? 5,500 tokens. Same information, 57x cheaper.**

The agent reads the page map, knows there are 4 elements, and acts immediately. No vision model. No guessing coordinates. No "I can see a button that says..."

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
  ⚡ rayo           ~1,500 tokens  ██░░░░░░░░░░░░░░░░░░  0.75% of 200k
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

## 🔧 The 7 tools

| Tool | Does |
|------|------|
| `rayo_navigate` | goto (with `wait_until`), reload, back, forward, tabs |
| `rayo_observe` | page_map (with element state + truncation metadata), text, screenshot |
| `rayo_interact` | click, hover, type, press (Enter/Tab/Escape), select, scroll |
| `rayo_batch` | N actions in 1 call (with `abort_on_failure`) |
| `rayo_cookie` | set, get, clear, save, load, **import from Chrome/Arc/Brave/Edge** |
| `rayo_network` | capture, block, mock requests |
| `rayo_profile` | performance data (ai_summary, json, markdown, chrome_trace) |

**Transparent auth:** Navigate to any authenticated page — ⚡ rayo auto-detects login walls, imports cookies from your real browser, and retries. Zero configuration.

## 🧪 Reproduce it

```bash
# Run the full competitor benchmark (vs Playwright + Puppeteer)
cd bench/competitors && npm install && npx tsx src/run-benchmarks.ts

# Run internal micro-benchmarks
cargo bench
```

## 🛠️ Development

```bash
cargo build --workspace          # Build
cargo test --workspace           # Test (needs Chrome)
cargo bench                      # Benchmark
cargo run --bin rayo-mcp         # Run
```

## License

MIT
