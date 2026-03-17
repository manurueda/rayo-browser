# Rules Enforcement: Implement remaining speed rules

## Why
Only 3 of 8 defined rules are actively enforced. The remaining 5 rules are defined in config but never checked: batching/combine-sequential, waits/no-fixed-timeout, waits/prefer-network-idle, contexts/prefer-reuse, screenshots/prefer-page-map.

## Solution
Implement the remaining rule checks in the rules engine to provide AI agents with actionable speed guidance.

## Scope

### rayo-rules
- `engine.rs` — Add `check_batch_opportunity()`, `check_page_map_preference()` methods
- Track sequential action count to warn when 3+ actions should be batched

### rayo-mcp
- `server.rs` — Wire new rule checks into tool dispatch

### Boundary
- Rules engine only — no browser behavior changes
- Violations are advisory (warn), not blocking
- Existing rule config format preserved
