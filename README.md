# rayo-browser

**The most token-efficient MCP browser automation tool.** Rust-powered, benchmark-proven, AI-native.

[![Star History Chart](https://api.star-history.com/svg?repos=manurueda/rayo-browser&type=Date)](https://www.star-history.com/#manurueda/rayo-browser&Date)

## Why rayo?

AI agents using Playwright for browser automation are painfully slow. Not because browsers are slow — because the automation stack is wrong for AI.

| Problem | Playwright | rayo-browser |
|---------|-----------|-------------|
| Relay server overhead | 2nd WebSocket hop, 326KB WS data | Direct CDP, 11KB WS data |
| Actions per MCP call | 1 action = 1 call | `rayo_batch`: 10 actions in 1 call |
| Page understanding | Screenshot: ~100k tokens | Page map: ~500 tokens |
| Auto-waits | Polling every 100-200ms | Event-driven (zero polling) |
| Tool descriptions | ~13,700 tokens | ~1,500 tokens |
| Speed guidance | None | Built-in rules with runtime feedback |

## Architecture

```
AI Agent (Claude, Cursor, etc.)
         │
    MCP (stdio/SSE)
         │
    ┌────▼────┐
    │ rayo-mcp │  ← 6 tools, ~1,500 tokens
    │  batch   │
    │  rules   │
    │  profiler │
    ├──────────┤
    │ rayo-core │  ← page maps, selector cache, event waits
    ├──────────┤
    │chromium- │  ← CDP protocol (don't rebuild, reuse)
    │  oxide   │
    └────┬─────┘
         │
    Chrome/Chromium
```

Built on [chromiumoxide](https://github.com/nickel-org/chromiumoxide) (1.2k stars, 1.2M+ downloads) for CDP. We don't rebuild solved problems — we build the AI-native layer on top.

## Quick Start

```bash
# Install
cargo install rayo-mcp

# Add to Claude Code
echo '{"mcpServers": {"rayo": {"command": "rayo-mcp"}}}' > ~/.claude.json

# Or run directly
rayo-mcp
```

## The 6 MCP Tools

| Tool | Purpose | Tokens |
|------|---------|--------|
| `rayo_navigate` | goto, reload, back, forward | ~200 |
| `rayo_observe` | page_map, text, screenshot, a11y | ~300 |
| `rayo_interact` | click, type, select, scroll | ~250 |
| `rayo_batch` | execute multiple actions in 1 call | ~350 |
| `rayo_network` | intercept, block, modify requests | ~250 |
| `rayo_profile` | get profiling results | ~150 |
| **Total** | | **~1,500** |

## Key Innovation: Page Maps

Instead of screenshots (100k tokens) or raw HTML (50k tokens):

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

**~500 tokens. 200x more efficient than a screenshot.**

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

4 actions in 1 MCP call. Without batch: 4 round-trips through the LLM. **~4x faster.**

## Built-in Profiling (On by Default)

Every operation is timed. Get results with `rayo_profile`:

```
RAYO PROFILE (1.23s total)
  navigation: 450ms (36.6%) | 3 ops | avg 150ms | p95 200ms
  dom.read:   280ms (22.8%) | 8 ops | avg 35ms  | p95 50ms
  screenshot: 180ms (14.6%) | 2 ops | avg 90ms  | p95 95ms
  cdp.command: 200ms (16.3%) | 45 ops | avg 4.4ms| p95 8ms
SLOWEST: goto("wikipedia.org") 200ms
```

Export to Chrome DevTools trace format, JSON, or markdown.

## AI Speed Rules

Ships with built-in rules that guide AI agents toward fast patterns:

```
RAYO SPEED RULES:
- PREFER CSS selectors over XPath (2-10x faster)
- DO NOT screenshot after every action. Use page_map instead.
- DO NOT sleep. Use event-driven waits.
- BATCH 3+ sequential actions into rayo_batch
```

Runtime feedback in every response:
```json
{
  "_rayo": {
    "durationMs": 45,
    "violations": [{ "rule": "selectors/prefer-css", "suggestion": "Use div.foo instead of //div[@class='foo']" }]
  }
}
```

## Benchmarks (Real Numbers, Real Websites)

Benchmarked on darwin arm64, 5 iterations after 2 warmup, median values.

### Where rayo dominates: Token Efficiency

| Method | Adapter | Latency | Tokens | vs Screenshot |
|--------|---------|---------|--------|---------------|
| **page_map** | **rayo** | **<1ms** | **~94** | **58x fewer tokens** |
| text | all | <1ms | ~33 | — |
| screenshot | playwright | 17ms | ~5,500 | baseline |
| screenshot | puppeteer | 17ms | ~5,500 | baseline |

**Page maps are the killer feature.** An AI agent reads ~94 tokens instead of ~5,500 for a screenshot. That's **58x more token-efficient** — and it returns structured data with element IDs for direct interaction.

### Tool Description Token Cost (Context Window Impact)

| MCP Server | Tools | Est. Tokens | % of 200k Context |
|-----------|-------|-------------|-------------------|
| **rayo-browser** | **5** | **~1,500** | **0.75%** |
| Puppeteer MCP | 9 | ~4,500 | 2.25% |
| Playwright MCP | 22 | ~13,200 | 6.60% |

rayo uses **8.8x fewer tokens** for tool descriptions than Playwright MCP.

### Navigation Speed (Honest Numbers)

| Site | rayo | Playwright | Puppeteer |
|------|------|-----------|-----------|
| example.com | 21ms | 3ms | 15ms |
| wikipedia | 110ms | 68ms | 80ms |
| HN | 158ms | 76ms | 93ms |

Playwright is faster at raw navigation because it runs in-process (no MCP stdio overhead). rayo pays for the MCP transport layer. **We're optimizing for total AI workflow cost (tokens + latency + round-trips), not raw navigation speed.**

### DOM Extraction (HN)

| Adapter | Latency | Items | Method |
|---------|---------|-------|--------|
| rayo | 1ms | 229 elements | page_map (structured) |
| playwright | 1ms | 30 stories | $$eval (custom JS) |
| puppeteer | 8ms | 30 stories | $$eval (custom JS) |

rayo's page_map returns **all 229 interactive elements** in one call — no custom JS needed. Playwright/Puppeteer require agent-written `$$eval` queries.

### The Real Advantage: AI Workflow Efficiency

For an AI agent doing browser automation, what matters is:
1. **Tokens consumed** (page maps: 58x fewer than screenshots)
2. **Context window overhead** (tool descriptions: 8.8x fewer than Playwright)
3. **Round-trips to understand a page** (1 page_map call vs screenshot + parse)
4. **Actions per MCP call** (batch: N actions in 1 call vs N calls)

```bash
# Run benchmarks yourself
cd bench/competitors && npx tsx src/run-benchmarks.ts

# Run Rust criterion benchmarks
cargo bench
```

## Development

```bash
# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Run benchmarks
cargo bench

# Run MCP server
cargo run --bin rayo-mcp
```

## License

MIT
