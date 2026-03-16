# ⚡ rayo-browser

**The fastest MCP browser automation tool.** Rust-powered, benchmark-proven, AI-native.

AI agents waste 90%+ of their context window on screenshots and tool descriptions. ⚡ rayo fixes that.

## 🚀 Benchmarks

Real Claude Code workflows. Real websites. 10 iterations, warm browsers. [Run them yourself →](#run-benchmarks-yourself)

### End-to-End Speed + Token Cost

```
                              Speed                    Tokens
                        ─────────────────       ─────────────────────

Wikipedia Research
  ⚡ rayo              132ms  ██░░░░░░░░░        5,453  █░░░░░░░░░░░░░░░░░░░
  Playwright          204ms  ███░░░░░░░░      100,367  ████████████████████
                                1.5x faster                    95% fewer tokens

Form Fill + Submit
  ⚡ rayo              251ms  ████░░░░░░░        3,690  ███░░░░░░░░░░░░░░░░░
  Playwright          411ms  ██████░░░░░       23,311  ████████████████████
                                1.6x faster                    84% fewer tokens

HN Browse + Read
  ⚡ rayo              212ms  █░░░░░░░░░░        6,961  ██░░░░░░░░░░░░░░░░░░
  Playwright        3,723ms  ████████████      75,395  ████████████████████
                               17.6x faster                    91% fewer tokens
```

### Page Understanding

| Method | Adapter | Latency | ~Tokens |
|--------|---------|---------|--------|
| **page_map** | **⚡ rayo** | **<1ms** | **~97** |
| text | ⚡ rayo | <1ms | ~47 |
| screenshot | Playwright | 17ms | ~5,526 |
| screenshot | Puppeteer | 17ms | ~5,487 |

### Tool Description Token Cost

```
  ⚡ rayo-browser  ~1,500 tokens  ██░░░░░░░░░░░░░░░░░░  0.75% of context
  Puppeteer MCP    ~4,500 tokens  ███████░░░░░░░░░░░░░  2.25% of context
  Playwright MCP  ~13,200 tokens  ████████████████████  6.60% of context
```

## 📦 Install

**Claude Code** — paste this:

> Install rayo-browser: run `cargo install rayo-mcp` then `claude mcp add rayo -- rayo-mcp`. Then add a "rayo-browser" section to CLAUDE.md that says to use rayo MCP tools for all browser automation, always prefer `rayo_observe` with `page_map` mode over screenshots, use `rayo_batch` to combine 3+ sequential actions into a single call, and lists the 7 tools: `rayo_navigate`, `rayo_observe`, `rayo_interact`, `rayo_batch`, `rayo_cookie`, `rayo_network`, `rayo_profile`.

**Cursor, Windsurf, or any MCP client:**

```json
{
  "mcpServers": {
    "rayo": {
      "command": "rayo-mcp"
    }
  }
}
```

## 💡 Why ⚡ rayo?

AI agents using Playwright for browser automation are painfully slow. Not because browsers are slow — because the automation stack is wrong for AI.

| Problem | Playwright | ⚡ rayo-browser |
|---------|-----------|-------------|
| Actions per MCP call | 1 action = 1 call | `rayo_batch`: 10 actions in 1 call |
| Page understanding | Screenshot: ~5,500 tokens | Page map: ~97 tokens |
| Tool descriptions | 22 tools, ~13,200 tokens | 7 tools, ~1,500 tokens |
| Context window cost | 6.60% of 200k | 0.75% of 200k |

## 🧠 Key Innovation: Page Maps

Instead of screenshots (~5,500 tokens) or raw HTML (~50k tokens):

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

**~97 tokens. 57x more efficient than a screenshot.**

## ⚡ Key Innovation: Batch Actions

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

4 actions in 1 MCP call. Without batch: 4 round-trips through the LLM.

## 🔧 The 7 MCP Tools

| Tool | Purpose | Tokens |
|------|---------|--------|
| `rayo_navigate` | goto, reload, back, forward, new_tab, close_tab, list_tabs, switch_tab | ~300 |
| `rayo_observe` | page_map, text, screenshot | ~300 |
| `rayo_interact` | click, type, select, scroll | ~250 |
| `rayo_batch` | execute multiple actions in 1 call | ~350 |
| `rayo_cookie` | set, get, clear cookies | ~250 |
| `rayo_network` | capture, block, mock, requests | ~250 |
| `rayo_profile` | get profiling results | ~150 |
| **Total** | | **~1,500** |

## 📊 Built-in Profiling

Every operation is timed. Get results with `rayo_profile`:

```
RAYO PROFILE (3379.6ms total)
  navigation: 3379.6ms (84.4%) | 47 ops | avg 71.9ms | p95 167.0ms
  screenshot:  241.4ms  (6.0%) | 13 ops | avg 18.6ms | p95 25.1ms
  page_map:    140.4ms  (3.5%) | 77 ops | avg  1.8ms | p95  5.2ms
  batch:       113.3ms  (2.8%) |  1 ops | avg 113.3ms
  dom.mutate:  113.2ms  (2.8%) |  4 ops | avg 28.3ms | p95 34.1ms
SLOWEST: goto(wikipedia.org) 431.2ms
```

## 🛡️ AI Speed Rules

Ships with built-in rules that guide AI agents toward fast patterns:

```
RAYO SPEED RULES:
- PREFER CSS selectors over XPath (2-10x faster)
- DO NOT screenshot after every action. Use page_map instead.
- DO NOT sleep. Use event-driven waits.
- BATCH 3+ sequential actions into rayo_batch
```

## 🏗️ Architecture

```
AI Agent (Claude Code, Cursor, etc.)
         |
    MCP (stdio)
         |
    +----------+
    | rayo-mcp |  ← 7 tools, ~1,500 tokens
    |  batch   |
    |  rules   |
    |  profiler |
    +----------+
    | rayo-core |  ← page maps, selector cache, tabs, network
    +----------+
    |chromium- |  ← CDP protocol
    |  oxide   |
    +----+-----+
         |
    Chrome/Chromium
```

## 🧪 Run Benchmarks Yourself

```bash
# Internal micro-benchmarks (Criterion)
cargo bench

# Competitor comparison (vs Playwright + Puppeteer)
cd bench/competitors && npm install && npx tsx src/run-benchmarks.ts
```

## 🛠️ Development

```bash
cargo build --workspace          # Build all
cargo test --workspace           # Run all tests (needs Chrome)
cargo bench                      # Run criterion benchmarks
cargo run --bin rayo-mcp         # Start MCP server
cargo clippy --workspace         # Lint
cargo fmt --check --all          # Check formatting
```

## License

MIT

---

[![Star History Chart](https://api.star-history.com/svg?repos=manurueda/rayo-browser&type=Date)](https://www.star-history.com/#manurueda/rayo-browser&Date)
