# rayo-ui: Tasks

## Tasks

- [ ] Scaffold crate: Cargo.toml, lib.rs, main.rs (CLI binary), error types
- [ ] Implement YAML test definition parser with serde
- [ ] Define test definition types: TestSuite, TestCase, TestStep, Assertion
- [ ] Implement StepExecutor: navigate, click, type, select, scroll, hover, press, wait, batch
- [ ] Implement AssertionEngine: page_map_contains, text_contains, screenshot_matches, network_called
- [ ] Implement ResultCollector: per-step timing, pass/fail, error messages, artifacts (page maps, screenshots)
- [ ] Implement JSON report generator
- [ ] Implement HTML report generator (self-contained, embeds screenshots)
- [ ] Implement axum web server: REST API endpoints + WebSocket for live updates
- [ ] Implement CLI: `rayo-ui run [suite]`, `rayo-ui list`, `rayo-ui ui`
- [ ] Wire profiler spans: TestSuite, TestCase, TestStep, Assertion categories
- [ ] Integration tests: parse YAML → execute → collect results → generate report
- [ ] Add rayo-ui to workspace Cargo.toml
