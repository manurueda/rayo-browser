# rayo-ui: AI-native E2E test runner

## Why
Momentic charges enterprise SaaS prices for AI-powered E2E testing. rayo-browser already has the fastest browser automation layer — adding a test runner on top makes rayo a complete testing platform where the AI agent IS the visual intelligence (zero additional AI API cost). The test runner orchestrates: load test definitions, execute steps via rayo-core, evaluate assertions (page_map, text, screenshot), collect results, serve them via API for the web UI.

## Solution
New `rayo-ui` crate containing the test runner engine, YAML test definition parser, assertion engine, result collector, report generator, and an axum web server for UI communication. Also provides a CLI binary (`rayo-ui`) for running tests from terminal and CI.

## Architecture

```
  rayo-ui CLI / Web UI
       │
       ▼
  TestRunner
       │
       ├── YamlParser → load .rayo/tests/*.test.yaml
       │
       ├── StepExecutor → drives rayo-core (navigate, click, type, etc.)
       │
       ├── AssertionEngine
       │   ├── page_map_contains(selector, text, role)
       │   ├── text_contains(expected)
       │   ├── screenshot_matches(baseline_name, threshold)
       │   └── network_called(url_pattern, method)
       │
       ├── ResultCollector → per-step timing, pass/fail, artifacts
       │
       ├── ReportGenerator → JSON + HTML
       │
       └── WebServer (axum)
           ├── GET  /api/suites
           ├── GET  /api/results/{suite_id}
           ├── POST /api/run
           └── WS   /ws/live (real-time step updates)
```

## Test Definition Format

```yaml
name: Login Flow
viewport: { width: 1280, height: 720 }
setup:
  - navigate: https://app.example.com
steps:
  - name: Fill login form
    batch:
      - { action: type, selector: "input[name='email']", value: "test@example.com" }
      - { action: click, selector: "button[type='submit']" }
  - name: Verify dashboard
    wait: { selector: ".dashboard", timeout_ms: 5000 }
    assert:
      - page_map_contains: { text: "Welcome" }
      - screenshot: { name: "dashboard", threshold: 0.01 }
```

## Scope
- YAML test definition parser
- Step executor (all rayo-core actions)
- Assertion engine (page_map, text, screenshot, network)
- Result collector with per-step artifacts
- JSON + HTML report generation
- Axum web server with REST API + WebSocket
- CLI binary: `rayo-ui run`, `rayo-ui list`, `rayo-ui ui`
- Profiler integration

## Not in scope
- Web UI frontend (rayo-ui-ui change)
- Test recording/replay (Phase 2)
- Sharding/parallelism (Phase 2)
- AI-powered self-healing (agent handles this)
