## Benchmark Results (2026-03-15)

**System:** darwin arm64 | **Iterations:** 10 after 3 warmup | **Warm browsers** (no cold-start)

### Navigation Speed (warm browser)

| Site | rayo | Playwright | Puppeteer |
|------|------|-----------|----------|
| example.com | 20ms | 2ms | 13ms |
| wikipedia | 119ms | 70ms | 82ms |
| HN | 100ms | 76ms | 93ms |

### Page Understanding (Speed + Token Cost)

| Method | Adapter | Latency | ~Tokens |
|--------|---------|---------|--------|
| text | playwright | 1ms | ~33 |
| text | puppeteer | 1ms | ~33 |
| page_map | rayo | 0ms | ~120 |
| text | rayo | 0ms | ~47 |
| screenshot | playwright | 17ms | ~5526 |
| screenshot | puppeteer | 17ms | ~5487 |
| screenshot | rayo | 527ms | ~5487 |

### DOM Extraction (HN)

| Adapter | Latency | Items | ~Tokens |
|---------|---------|-------|--------|
| playwright | 1ms | 30 | ~928 |
| puppeteer | 8ms | 30 | ~928 |
| rayo | 2ms | 229 | ~9416 |

### 🤖 AI Agent Session Simulations (Real Claude Code Patterns)

These simulate actual Claude Code workflows — the TOTAL cost of tokens + latency + tool calls.

#### Wikipedia Research

| Metric | Playwright MCP | rayo-browser | Advantage |
|--------|---------------|-------------|----------|
| Latency | 404ms | 210ms | rayo 1.9x faster |
| Tool calls | 4 | 3 | **25% fewer** |
| Total tokens | 100,367 | 65,744 | **34% fewer** |

#### Form Fill

| Metric | Playwright MCP | rayo-browser | Advantage |
|--------|---------------|-------------|----------|
| Latency | 247ms | 758ms | PW 3.1x faster |
| Tool calls | 8 | 5 | **38% fewer** |
| Total tokens | 23,311 | 4,578 | **80% fewer** |

#### Hn Browse

| Metric | Playwright MCP | rayo-browser | Advantage |
|--------|---------------|-------------|----------|
| Latency | 1200ms | 475ms | rayo 2.5x faster |
| Tool calls | 5 | 4 | **20% fewer** |
| Total tokens | 77,538 | 21,139 | **73% fewer** |

### Tool Description Token Cost

| MCP Server | Tools | Tokens | % of 200k Context |
|-----------|-------|--------|-------------------|
| playwright | 22 | ~13,200 | 6.60% |
| puppeteer | 9 | ~4,500 | 2.25% |
| rayo | 5 | ~1,500 | 0.75% |

---
*Warm browsers, 10 iterations after 3 warmup, median values.*
