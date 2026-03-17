# Test Coverage Expansion: Error paths, edge cases, and integration gaps

## Why
Current test suite (41 tests) covers happy paths well but has zero tests for error paths, edge cases, or concurrent operations. Estimated coverage ~35-40%. Production failures will crash or hang silently without error path tests.

## Solution
Add targeted tests for:
1. Error paths: invalid selectors, batch mixed success/failure, element not found
2. Edge cases: page with 100+ elements (truncation), empty page, stale selectors
3. Integration: network capture with real CDP events, cookie round-trip

## Scope

### rayo-core
- `tests/integration_test.rs` — Add error path tests: invalid selector click, batch with failing actions
- New test fixture: `tests/fixtures/many_elements.html` — page with 100+ interactive elements

### rayo-mcp
- `tests/mcp_integration_test.rs` — Add batch mixed success/failure test, empty page page_map test

### Boundary
- Tests only — no production code changes
- Uses existing fixture server infrastructure
- Single browser instance pattern preserved
