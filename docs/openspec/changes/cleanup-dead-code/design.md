# Design: Dead Code Cleanup

## Files to delete
- `crates/rayo-mcp/src/tools/cookie.rs` — duplicate handler, never imported by server.rs
- `crates/rayo-mcp/src/tools/navigate.rs` — unused NavigateInput/Action/Output types
- `crates/rayo-mcp/src/tools/observe.rs` — unused ObserveInput/Mode/Output types
- `crates/rayo-mcp/src/tools/interact.rs` — unused InteractInput/Action/Output types
- `crates/rayo-mcp/src/tools/batch.rs` — unused BatchInput/Output types
- `crates/rayo-mcp/src/tools/network.rs` — unused NetworkInput/Mode/MockResponse types
- `crates/rayo-mcp/src/tools/profile.rs` — unused ProfileInput/Format/Output types

## Verification
- `cargo build --workspace` passes
- `cargo test --workspace --lib` passes
- `grep -rn "tools::" crates/rayo-mcp/src/` confirms no broken imports
