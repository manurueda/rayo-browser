# rayo-browser

The fastest MCP browser automation tool. Rust-powered, benchmark-proven, AI-native.

## Architecture

```
AI Agent → MCP (stdio) → rayo-mcp → rayo-core → chromiumoxide → Chrome
                           │            │
                        rayo-rules   rayo-profiler
```

4 crates:
- `rayo-profiler` — profiling (no deps on other rayo crates)
- `rayo-core` — browser intelligence (page maps, batch, cache, waits)
- `rayo-rules` — speed rules engine
- `rayo-mcp` — MCP server binary (5 tools)

## Commands

```bash
cargo build --workspace          # Build all
cargo test --workspace           # Run all tests (needs Chrome)
cargo bench                      # Run criterion benchmarks
cargo run --bin rayo-mcp         # Start MCP server
```

## Integration tests require Chrome

Tests that need a browser will auto-skip if Chrome isn't found.
Install Chrome or Chromium to run integration tests.

## Key conventions

- chromiumoxide handles CDP — do not rewrite CDP protocol handling
- Page maps are the primary page representation (~500 tokens vs 100k for screenshots)
- Batch execution is the primary speed optimization for AI agents
- Profiling is on by default
- All browser integration tests share one browser instance (avoid Chrome process conflicts)
