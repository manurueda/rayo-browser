# Design: Fix Profile Format

## Change handle_profile signature
From: `handle_profile(profiler: &Arc<Profiler>)`
To: `handle_profile(profiler: &Arc<Profiler>, params: &serde_json::Map<String, Value>)`

## Implementation
```rust
let format = params.get("format").and_then(|v| v.as_str()).unwrap_or("ai_summary");
let text = match format {
    "json" => profiler.export_json(),
    "markdown" => profiler.export_markdown(),
    "chrome_trace" => profiler.export_chrome_trace(),
    _ => profiler.export_ai_summary(),
};
```

## Server.rs update
Pass `&params` to handle_profile call site.
