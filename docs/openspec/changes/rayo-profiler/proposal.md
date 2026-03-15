# rayo-profiler: Built-in Profiling Engine

## Problem
Browser automation tools provide no visibility into where time is spent. Users claim "it's slow" but can't pinpoint whether the bottleneck is CDP round-trips, selector resolution, JS evaluation, or network waits. AI agents have no structured way to analyze performance.

## Solution
A profiling crate that instruments every operation with nanosecond-precision timing, exports to standard formats (Chrome trace, JSON, markdown), and produces AI-readable summaries. Enabled by default (per user request), with compile-time elimination available via feature flag for absolute zero overhead.

## Key Design Decisions
- **On by default** — profiling is always active unless explicitly compiled out
- **Ring buffer storage** — bounded memory (100k spans, ~20MB max), no unbounded growth
- **Standard exports** — Chrome DevTools trace format (chrome://tracing), JSON, markdown, AI summary
- **Hybrid LLM + programmatic** — AI summary format designed for LLM consumption, structured data for programmatic analysis
- **Zero-cost disable** — `#[cfg(not(feature = "profiler"))]` compiles away all instrumentation

## Scope
- `crates/rayo-profiler/src/lib.rs` — public API, `profile_span!` macro, `Profiler` trait
- `crates/rayo-profiler/src/collector.rs` — span collection, ring buffer, timing
- `crates/rayo-profiler/src/stats.rs` — statistical analysis (mean, median, p95, p99, stddev)
- `crates/rayo-profiler/src/export.rs` — JSON, Chrome trace, markdown, AI summary exporters

## Not in Scope
- Dashboard UI (future module)
- Flame graph generation (future module)
- Comparison mode vs Playwright (future module)
