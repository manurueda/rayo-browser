## Benchmark Results (2026-03-15)

**System:** darwin arm64 | **Iterations:** 10 after 3 warmup | **Warm browsers** (no cold-start)

### Navigation Speed (warm browser)

| Site | rayo | Playwright | Puppeteer |
|------|------|-----------|----------|
| example.com | 24ms | 3ms | 18ms |
| wikipedia | 130ms | 78ms | 91ms |
| HN | 97ms | 74ms | 92ms |

### Page Understanding (Speed + Token Cost)

| Method | Adapter | Latency | ~Tokens |
|--------|---------|---------|--------|
| text | playwright | 1ms | ~33 |
| text | puppeteer | 1ms | ~33 |
| page_map | rayo | 0ms | ~120 |
| text | rayo | 0ms | ~47 |
| screenshot | playwright | 17ms | ~5526 |
| screenshot | puppeteer | 17ms | ~5487 |
| screenshot | rayo | 528ms | ~5487 |

### DOM Extraction (HN)

| Adapter | Latency | Items | ~Tokens |
|---------|---------|-------|--------|
| playwright | 2ms | 30 | ~935 |
| puppeteer | 8ms | 30 | ~935 |
| rayo | 2ms | 229 | ~9417 |

### 🤖 AI Agent Session Simulations (Real Claude Code Patterns)

These simulate actual Claude Code workflows — the TOTAL cost of tokens + latency + tool calls.

#### Wikipedia Research

| Metric | Playwright MCP | rayo-browser | Advantage |
|--------|---------------|-------------|----------|
| Latency | 406ms | 208ms | rayo 2.0x faster |
| Tool calls | 4 | 3 | **25% fewer** |
| Total tokens | 100,369 | 43,290 | **57% fewer** |

#### Form Fill

| Metric | Playwright MCP | rayo-browser | Advantage |
|--------|---------------|-------------|----------|
| Latency | 427ms | 646ms | PW 1.5x faster |
| Tool calls | 8 | 5 | **38% fewer** |
| Total tokens | 23,311 | 3,753 | **84% fewer** |

#### Hn Browse

| Metric | Playwright MCP | rayo-browser | Advantage |
|--------|---------------|-------------|----------|
| Latency | 1439ms | 1105ms | rayo 1.3x faster |
| Tool calls | 5 | 4 | **20% fewer** |
| Total tokens | 77,523 | 21,142 | **73% fewer** |

### Tool Description Token Cost

| MCP Server | Tools | Tokens | % of 200k Context |
|-----------|-------|--------|-------------------|
| playwright | 22 | ~13,200 | 6.60% |
| puppeteer | 9 | ~4,500 | 2.25% |
| rayo | 5 | ~1,500 | 0.75% |

---
*Warm browsers, 10 iterations after 3 warmup, median values.*
