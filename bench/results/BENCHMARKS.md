## Benchmark Results (2026-03-15)

**System:** darwin arm64 | **Iterations:** 10 after 3 warmup | **Warm browsers** (no cold-start)

### Navigation Speed (warm browser)

| Site | rayo | Playwright | Puppeteer |
|------|------|-----------|----------|
| example.com | 19ms | 2ms | 13ms |
| wikipedia | 102ms | 68ms | 80ms |
| HN | 121ms | 80ms | 99ms |

### Page Understanding (Speed + Token Cost)

| Method | Adapter | Latency | ~Tokens |
|--------|---------|---------|--------|
| text | playwright | 1ms | ~33 |
| text | puppeteer | 1ms | ~33 |
| page_map | rayo | 0ms | ~120 |
| text | rayo | 0ms | ~47 |
| screenshot | playwright | 17ms | ~5526 |
| screenshot | puppeteer | 17ms | ~5487 |
| screenshot | rayo | 526ms | ~4572 |

### DOM Extraction (HN)

| Adapter | Latency | Items | ~Tokens |
|---------|---------|-------|--------|
| playwright | 1ms | 30 | ~948 |
| puppeteer | 8ms | 30 | ~948 |
| rayo | 1ms | 50 | ~2029 |

### 🤖 AI Agent Session Simulations (Real Claude Code Patterns)

These simulate actual Claude Code workflows — the TOTAL cost of tokens + latency + tool calls.

#### Wikipedia Research

| Metric | Playwright MCP | rayo-browser | Advantage |
|--------|---------------|-------------|----------|
| Latency | 417ms | 194ms | rayo 2.2x faster |
| Tool calls | 4 | 3 | **25% fewer** |
| Total tokens | 100,367 | 6,735 | **93% fewer** |

#### Form Fill

| Metric | Playwright MCP | rayo-browser | Advantage |
|--------|---------------|-------------|----------|
| Latency | 308ms | 636ms | PW 2.1x faster |
| Tool calls | 8 | 5 | **38% fewer** |
| Total tokens | 23,311 | 4,581 | **80% fewer** |

#### Hn Browse

| Metric | Playwright MCP | rayo-browser | Advantage |
|--------|---------------|-------------|----------|
| Latency | 1289ms | 1000ms | rayo 1.3x faster |
| Tool calls | 5 | 4 | **20% fewer** |
| Total tokens | 76,757 | 5,669 | **93% fewer** |

### Tool Description Token Cost

| MCP Server | Tools | Tokens | % of 200k Context |
|-----------|-------|--------|-------------------|
| playwright | 22 | ~13,200 | 6.60% |
| puppeteer | 9 | ~4,500 | 2.25% |
| rayo | 5 | ~1,500 | 0.75% |

---
*Warm browsers, 10 iterations after 3 warmup, median values.*
