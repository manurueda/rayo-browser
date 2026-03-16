# Design: Profiler Safety

## Pattern
Replace:
```rust
let mut c = self.collector.lock().unwrap();
```
With:
```rust
let Ok(mut c) = self.collector.lock() else { return default; };
```

Where `default` is:
- `start_span()` → return a dummy SpanId (0)
- `spans()` → return empty Vec
- `reset()` → no-op return
- `span_count()` → return 0

This matches the SpanGuard Drop impl pattern (lib.rs line 96):
```rust
if let Ok(mut c) = self.collector.lock() {
    c.end_span(self.id);
}
```
