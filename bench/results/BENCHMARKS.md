## Benchmark Results (2026-03-16)

**System:** darwin arm64 | **Iterations:** 10 after 3 warmup | **Warm browsers** (no cold-start)

### Navigation Speed (warm browser)

| Site | rayo | Playwright | Puppeteer |
|------|------|-----------|----------|
| example.com | 3ms | 3ms | 13ms |
| wikipedia | 72ms | 69ms | 81ms |
| HN | 81ms | 75ms | 89ms |

### Page Understanding (Speed + Token Cost)

| Method | Adapter | Latency | ~Tokens |
|--------|---------|---------|--------|
| text | playwright | 1ms | ~33 |
| text | puppeteer | 1ms | ~33 |
| page_map | rayo | 0ms | ~97 |
| text | rayo | 0ms | ~47 |
| screenshot | playwright | 17ms | ~5526 |
| screenshot | puppeteer | 17ms | ~5487 |
| screenshot | rayo | 17ms | ~4572 |

### DOM Extraction (HN)

| Adapter | Latency | Items | ~Tokens |
|---------|---------|-------|--------|
| playwright | 1ms | 30 | ~935 |
| puppeteer | 7ms | 30 | ~935 |
| rayo | 1ms | 50 | ~1466 |

### 🤖 AI Agent Session Simulations (Real Claude Code Patterns)

These simulate actual Claude Code workflows — the TOTAL cost of tokens + latency + tool calls.

#### Wikipedia Research

| Metric | Playwright MCP | rayo-browser | Advantage |
|--------|---------------|-------------|----------|
| Latency | 204ms | 132ms | rayo 1.5x faster |
| Tool calls | 4 | 3 | **25% fewer** |
| Total tokens | 100,367 | 5,453 | **95% fewer** |

#### Form Fill

| Metric | Playwright MCP | rayo-browser | Advantage |
|--------|---------------|-------------|----------|
| Latency | 411ms | 251ms | rayo 1.6x faster |
| Tool calls | 8 | 4 | **50% fewer** |
| Total tokens | 23,311 | 3,690 | **84% fewer** |

#### Hn Browse

| Metric | Playwright MCP | rayo-browser | Advantage |
|--------|---------------|-------------|----------|
| Latency | 3723ms | 212ms | rayo 17.6x faster |
| Tool calls | 5 | 4 | **20% fewer** |
| Total tokens | 75,395 | 6,961 | **91% fewer** |

### Tool Description Token Cost

| MCP Server | Tools | Tokens | % of 200k Context |
|-----------|-------|--------|-------------------|
| playwright | 22 | ~13,200 | 6.60% |
| puppeteer | 9 | ~4,500 | 2.25% |
| rayo | 5 | ~1,500 | 0.75% |

### rayo-browser Internal Profile

Where rayo spends its time (built-in profiler, always on):

```
## Rayo Profile (3379.6ms total)

| Category | Total | Count | Avg | p95 | % |
|----------|-------|-------|-----|-----|---|
| navigation | 3379.6ms | 47 | 71.9ms | 167.0ms | 84.4% |
| screenshot | 241.4ms | 13 | 18.6ms | 25.1ms | 6.0% |
| page_map | 140.4ms | 77 | 1.8ms | 5.2ms | 3.5% |
| batch | 113.3ms | 1 | 113.3ms | 113.3ms | 2.8% |
| dom.mutate | 113.2ms | 4 | 28.3ms | 34.1ms | 2.8% |
| dom.read | 18.0ms | 15 | 1.2ms | 6.8ms | 0.4% |

### Slowest Operations

| Operation | Duration |
|-----------|----------|
| goto(https://en.wikipedia.org/wiki/Web_browser) | 431.2ms |
| goto(https://news.ycombinator.com) | 376.4ms |
| goto(https://example.com) | 167.0ms |
| goto(https://httpbin.org/forms/post) | 133.6ms |
| batch(4) | 113.3ms |

```

---
*Warm browsers, 10 iterations after 3 warmup, median values.*
