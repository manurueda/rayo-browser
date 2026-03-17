## Benchmark Results (2026-03-17)

**System:** darwin arm64 | **Iterations:** 10 after 3 warmup | **Warm browsers** (no cold-start)

### Navigation Speed (warm browser)

| Site | rayo | Playwright | Puppeteer |
|------|------|-----------|----------|
| example.com | 23ms | 18ms | 32ms |
| wikipedia | 76ms | 72ms | 84ms |
| HN | 75ms | 75ms | 89ms |

### Page Understanding (Speed + Token Cost)

| Method | Adapter | Latency | ~Tokens |
|--------|---------|---------|--------|
| text | playwright | 1ms | ~33 |
| text | puppeteer | 1ms | ~33 |
| page_map | rayo | 0ms | ~97 |
| text | rayo | 0ms | ~47 |
| screenshot | playwright | 17ms | ~5526 |
| screenshot | puppeteer | 17ms | ~5487 |
| screenshot | rayo | 18ms | ~4572 |

### DOM Extraction (HN)

| Adapter | Latency | Items | ~Tokens |
|---------|---------|-------|--------|
| playwright | 1ms | 30 | ~874 |
| puppeteer | 8ms | 30 | ~874 |
| rayo | 1ms | 50 | ~1468 |

### 🤖 AI Agent Session Simulations (Real Claude Code Patterns)

These simulate actual Claude Code workflows — the TOTAL cost of tokens + latency + tool calls.

#### Wikipedia Research

| Metric | Playwright MCP | rayo-browser | Advantage |
|--------|---------------|-------------|----------|
| Latency | 229ms | 136ms | rayo 1.7x faster |
| Tool calls | 4 | 3 | **25% fewer** |
| Total tokens | 100,367 | 5,597 | **94% fewer** |

#### Form Fill

| Metric | Playwright MCP | rayo-browser | Advantage |
|--------|---------------|-------------|----------|
| Latency | 346ms | 190ms | rayo 1.8x faster |
| Tool calls | 8 | 4 | **50% fewer** |
| Total tokens | 23,311 | 3,685 | **84% fewer** |

#### Hn Browse

| Metric | Playwright MCP | rayo-browser | Advantage |
|--------|---------------|-------------|----------|
| Latency | 1274ms | 199ms | rayo 6.4x faster |
| Tool calls | 5 | 4 | **20% fewer** |
| Total tokens | 75,914 | 7,058 | **91% fewer** |

### Tool Description Token Cost

| MCP Server | Tools | Tokens | % of 200k Context |
|-----------|-------|--------|-------------------|
| playwright | 22 | ~13,200 | 6.60% |
| puppeteer | 9 | ~4,500 | 2.25% |
| rayo | 5 | ~1,500 | 0.75% |

### rayo-browser Internal Profile

Where rayo spends its time (built-in profiler, always on):

```
## Rayo Profile (3633.5ms total)

| Category | Total | Count | Avg | p95 | % |
|----------|-------|-------|-----|-----|---|
| auth | 3633.5ms | 46 | 79.0ms | 178.0ms | 47.4% |
| navigation | 3457.3ms | 47 | 73.6ms | 173.4ms | 45.1% |
| screenshot | 243.6ms | 13 | 18.7ms | 25.0ms | 3.2% |
| page_map | 170.2ms | 77 | 2.2ms | 11.7ms | 2.2% |
| batch | 75.1ms | 1 | 75.1ms | 75.1ms | 1.0% |
| dom.mutate | 75.1ms | 4 | 18.8ms | 23.2ms | 1.0% |
| dom.read | 15.3ms | 15 | 1.0ms | 5.2ms | 0.2% |

### Slowest Operations

| Operation | Duration |
|-----------|----------|
| goto_with_auto_auth(https://en.wikipedia.org/wiki/Web_browser) | 453.3ms |
| goto(https://en.wikipedia.org/wiki/Web_browser) | 443.0ms |
| goto_with_auto_auth(https://news.ycombinator.com) | 336.0ms |
| goto(https://news.ycombinator.com) | 333.9ms |
| goto_with_auto_auth(https://example.com) | 178.0ms |
VERSION: rayo-mcp v0.1.0

```

---
*Warm browsers, 10 iterations after 3 warmup, median values.*
