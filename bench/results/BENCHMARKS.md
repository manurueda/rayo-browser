## Benchmark Results (2026-03-15)

**System:** darwin arm64 | **Iterations:** 10 after 3 warmup | **Warm browsers** (no cold-start)

### Navigation Speed (warm browser)

| Site | rayo | Playwright | Puppeteer |
|------|------|-----------|----------|
| example.com | 21ms | 3ms | 15ms |
| wikipedia | 101ms | 72ms | 85ms |
| HN | 96ms | 75ms | 233ms |

### Page Understanding (Speed + Token Cost)

| Method | Adapter | Latency | ~Tokens |
|--------|---------|---------|--------|
| text | playwright | 1ms | ~33 |
| text | puppeteer | 1ms | ~33 |
| page_map | rayo | 0ms | ~120 |
| text | rayo | 0ms | ~47 |
| screenshot | playwright | 17ms | ~5526 |
| screenshot | puppeteer | 17ms | ~5487 |
| screenshot | rayo | 525ms | ~5487 |

### DOM Extraction (HN)

| Adapter | Latency | Items | ~Tokens |
|---------|---------|-------|--------|
| playwright | 1ms | 30 | ~961 |
| puppeteer | 8ms | 30 | ~961 |
| rayo | 2ms | 229 | ~9441 |

### 🤖 AI Agent Session Simulations (Real Claude Code Patterns)

These simulate actual Claude Code workflows — the TOTAL cost of tokens + latency + tool calls.

#### Wikipedia Research

| Metric | Playwright MCP | rayo-browser | Advantage |
|--------|---------------|-------------|----------|
| Latency | 219ms | 186ms | rayo 1.2x faster |
| Tool calls | 4 | 3 | **25% fewer** |
| Total tokens | 100,367 | 22,452 | **78% fewer** |

#### Form Fill

| Metric | Playwright MCP | rayo-browser | Advantage |
|--------|---------------|-------------|----------|
| Latency | 392ms | 646ms | PW 1.6x faster |
| Tool calls | 8 | 5 | **38% fewer** |
| Total tokens | 23,311 | 2,990 | **87% fewer** |

#### Hn Browse

| Metric | Playwright MCP | rayo-browser | Advantage |
|--------|---------------|-------------|----------|
| Latency | 945ms | 472ms | rayo 2.0x faster |
| Tool calls | 5 | 4 | **20% fewer** |
| Total tokens | 77,242 | 11,770 | **85% fewer** |

### Tool Description Token Cost

| MCP Server | Tools | Tokens | % of 200k Context |
|-----------|-------|--------|-------------------|
| playwright | 22 | ~13,200 | 6.60% |
| puppeteer | 9 | ~4,500 | 2.25% |
| rayo | 5 | ~1,500 | 0.75% |

---
*Warm browsers, 10 iterations after 3 warmup, median values.*
