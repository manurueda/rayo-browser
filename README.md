# rayo-browser

**The most token-efficient MCP browser automation tool.** Rust-powered, benchmark-proven, AI-native.

## Install

Open Claude Code and paste this:

> Install rayo-browser: run `cargo install rayo-mcp` then `claude mcp add rayo -- rayo-mcp`. Then add a "rayo-browser" section to CLAUDE.md that says to use rayo MCP tools for all browser automation, always prefer `rayo_observe` with `page_map` mode over screenshots, use `rayo_batch` to combine 3+ sequential actions into a single call, and lists the 7 tools: `rayo_navigate`, `rayo_observe`, `rayo_interact`, `rayo_batch`, `rayo_cookie`, `rayo_network`, `rayo_profile`.

For Cursor, Windsurf, or other MCP clients — add to your MCP config:

```json
{
  "mcpServers": {
    "rayo": {
      "command": "rayo-mcp"
    }
  }
}
```

## Why rayo?

AI agents using Playwright for browser automation are painfully slow. Not because browsers are slow — because the automation stack is wrong for AI.

| Problem | Playwright | rayo-browser |
|---------|-----------|-------------|
| Actions per MCP call | 1 action = 1 call | `rayo_batch`: 10 actions in 1 call |
| Page understanding | Screenshot: ~5,500 tokens | Page map: ~120 tokens |
| Tool descriptions | 22 tools, ~13,200 tokens | 7 tools, ~1,500 tokens |
| Context window cost | 6.60% of 200k | 0.75% of 200k |

## Architecture

```
AI Agent (Claude Code, Cursor, etc.)
         |
    MCP (stdio)
         |
    +----------+
    | rayo-mcp |  <- 7 tools, ~1,500 tokens
    |  batch   |
    |  rules   |
    |  profiler |
    +----------+
    | rayo-core |  <- page maps, selector cache, tabs, network
    +----------+
    |chromium- |  <- CDP protocol
    |  oxide   |
    +----+-----+
         |
    Chrome/Chromium
```

## The 7 MCP Tools

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

## Key Innovation: Page Maps

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

**~120 tokens. 46x more efficient than a screenshot.**

## Key Innovation: Batch Actions

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

## Benchmarks

Real numbers from real websites. 10 iterations, 3 warmup, warm browsers.

### AI Agent Sessions (Real Claude Code Patterns)

```
Wikipedia Research        rayo 22,452 tokens  ██░░░░░░░░░░░░░░░░░░  78% fewer
                    Playwright 100,367 tokens  ████████████████████

Form Fill + Submit        rayo 2,990 tokens   ███░░░░░░░░░░░░░░░░░  87% fewer
                    Playwright 23,311 tokens   ████████████████████

HN Browse + Read          rayo 11,770 tokens  ███░░░░░░░░░░░░░░░░░  85% fewer
                    Playwright 77,242 tokens   ████████████████████
```

### Page Understanding

| Method | Adapter | Latency | ~Tokens |
|--------|---------|---------|--------|
| **page_map** | **rayo** | **<1ms** | **~120** |
| text | rayo | <1ms | ~47 |
| screenshot | playwright | 17ms | ~5,526 |

### Tool Description Token Cost

```
  rayo-browser   ~1,500 tokens  ██░░░░░░░░░░░░░░░░░░  0.75% of context
  Puppeteer MCP  ~4,500 tokens  ███████░░░░░░░░░░░░░  2.25% of context
  Playwright MCP ~13,200 tokens ████████████████████  6.60% of context
```

```bash
# Run benchmarks yourself
cd bench/competitors && npx tsx src/run-benchmarks.ts
```

## Built-in Profiling

Every operation is timed. Get results with `rayo_profile`:

```
RAYO PROFILE (1.23s total)
  navigation: 450ms (36.6%) | 3 ops | avg 150ms | p95 200ms
  dom.read:   280ms (22.8%) | 8 ops | avg 35ms  | p95 50ms
  screenshot: 180ms (14.6%) | 2 ops | avg 90ms  | p95 95ms
  cdp.command: 200ms (16.3%) | 45 ops | avg 4.4ms| p95 8ms
SLOWEST: goto("wikipedia.org") 200ms
```

## AI Speed Rules

Ships with built-in rules that guide AI agents toward fast patterns:

```
RAYO SPEED RULES:
- PREFER CSS selectors over XPath (2-10x faster)
- DO NOT screenshot after every action. Use page_map instead.
- DO NOT sleep. Use event-driven waits.
- BATCH 3+ sequential actions into rayo_batch
```

## Development

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
