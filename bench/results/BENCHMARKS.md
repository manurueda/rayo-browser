## Benchmark Results (2026-03-16)

**System:** darwin arm64 | **Iterations:** 10 after 3 warmup | **Warm browsers** (no cold-start)

### Navigation Speed (warm browser)

| Site | rayo | Playwright | Puppeteer |
|------|------|-----------|----------|
| example.com | 3ms | 3ms | 13ms |
| wikipedia | 71ms | 67ms | 80ms |
| HN | 78ms | 75ms | 92ms |

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
| playwright | 1ms | 30 | ~953 |
| puppeteer | 7ms | 30 | ~953 |
| rayo | 1ms | 50 | ~1457 |

### 🤖 AI Agent Session Simulations (Real Claude Code Patterns)

These simulate actual Claude Code workflows — the TOTAL cost of tokens + latency + tool calls.

#### Wikipedia Research

| Metric | Playwright MCP | rayo-browser | Advantage |
|--------|---------------|-------------|----------|
| Latency | 190ms | 131ms | rayo 1.5x faster |
| Tool calls | 4 | 3 | **25% fewer** |
| Total tokens | 92,604 | 5,453 | **94% fewer** |

#### Form Fill

| Metric | Playwright MCP | rayo-browser | Advantage |
|--------|---------------|-------------|----------|
| Latency | 236ms | 215ms | rayo 1.1x faster |
| Tool calls | 8 | 4 | **50% fewer** |
| Total tokens | 23,310 | 3,687 | **84% fewer** |

#### Hn Browse

| Metric | Playwright MCP | rayo-browser | Advantage |
|--------|---------------|-------------|----------|
| Latency | 4428ms | 195ms | rayo 22.8x faster |
| Tool calls | 5 | 4 | **20% fewer** |
| Total tokens | 74,162 | 6,942 | **91% fewer** |

### Tool Description Token Cost

| MCP Server | Tools | Tokens | % of 200k Context |
|-----------|-------|--------|-------------------|
| playwright | 22 | ~13,200 | 6.60% |
| puppeteer | 9 | ~4,500 | 2.25% |
| rayo | 5 | ~1,500 | 0.75% |

---
*Warm browsers, 10 iterations after 3 warmup, median values.*
