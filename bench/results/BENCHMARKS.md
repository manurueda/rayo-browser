## Benchmark Results (2026-03-15)

**System:** darwin arm64
**Iterations:** 5 (after 2 warmup)

### Navigation Speed

| Site | rayo | Playwright | Puppeteer | rayo vs PW | rayo vs Pptr |
|------|------|-----------|-----------|------------|-------------|
| example.com | 21ms | 3ms | 15ms | **0.12x** | **0.72x** |
| wikipedia | 110ms | 68ms | 80ms | **0.62x** | **0.73x** |
| HN | 158ms | 76ms | 93ms | **0.48x** | **0.59x** |

### Page Understanding (Speed + Token Cost)

| Method | Adapter | Latency | Tokens | Token Efficiency |
|--------|---------|---------|--------|------------------|
| **page_map** | rayo | 0ms | ~94 | **Best** |
| text | rayo | 0ms | ~33 | |
| screenshot | rayo | 524ms | ~5487 | |
| text | playwright | 1ms | ~33 | |
| screenshot | playwright | 17ms | ~5526 | |
| text | puppeteer | 1ms | ~33 | |
| screenshot | puppeteer | 17ms | ~5487 | |

### DOM Extraction (HN Stories)

| Adapter | Latency | Items | Method |
|---------|---------|-------|--------|
| rayo | 1ms | 229 | page_map |
| playwright | 1ms | 30 | $$eval |
| puppeteer | 8ms | 30 | $$eval |

### Multi-Step Workflow (navigate + extract + screenshot)

| Adapter | Latency | MCP Calls | Method |
|---------|---------|-----------|--------|
| rayo | 552ms | 1 (batch) | rayo_batch |
| playwright | 25ms | 3 (sequential) | 3x tool calls |
| puppeteer | 47ms | 3 (sequential) | 3x tool calls |

### Tool Description Token Cost (Context Window Impact)

| MCP Server | Tools | Est. Tokens | % of 200k Context |
|-----------|-------|-------------|-------------------|
| **rayo-browser** | 5 | ~1500 | 0.75% |
| Puppeteer MCP | 9 | ~4500 | 2.25% |
| Playwright MCP | 22 | ~13200 | 6.60% |

---
*Benchmarks run on real public websites. 5 iterations after 2 warmup. Median values reported.*
