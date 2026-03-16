# Dead Code Cleanup: Remove duplicated cookie handler, dead tool files, fix JPEG description

## Why
The rayo-mcp crate has accumulated dead code during development:
1. `tools/cookie.rs` (131 lines) duplicates `tools/mod.rs` cookie handler — never called
2. Six tool files (navigate.rs, observe.rs, interact.rs, batch.rs, network.rs, profile.rs) contain only unused type definitions — all handler logic lives in mod.rs
3. The rayo_observe tool description says "base64 PNG" but screenshots are now JPEG

## Solution
Delete dead files. Fix the tool description. No behavioral changes.

## Scope

### rayo-mcp
- Delete `tools/cookie.rs` — duplicate of mod.rs:243-343
- Delete `tools/navigate.rs`, `tools/observe.rs`, `tools/interact.rs`, `tools/batch.rs`, `tools/network.rs`, `tools/profile.rs` — unused type defs
- Remove `pub mod` declarations for deleted files from `tools/mod.rs`
- `server.rs` line 225 — change "base64 PNG" to "base64 JPEG"

## Not in scope
- Restructuring tool handlers (separate change if desired)
- Adding the type definitions back in a consolidated types.rs
