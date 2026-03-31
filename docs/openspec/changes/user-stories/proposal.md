# User Stories: Non-Visual Testing + Discovery for Non-Developers

## Why

rayo-ui tests at the flow level — individual interactions (login, search, form submit). Two problems:

1. **No non-visual assertions or deterministic network checks.** The `network_called` assertion is a stub. Can't verify API calls happened, JS state changed, console is clean, cookies are set, or elements are disabled/checked. There is also no suite-scoped way to mock network responses for repeatable flows.

2. **No user story concept.** Flows are isolated. Non-developers (PMs, QA) can't see "Customer can search and purchase" as a single journey. No way to chain flows or present results without CSS selectors.

## Solution

A **User Story** layer on top of existing flows:

- **New assertion types**: `js_eval`, `element_state`, `no_console_errors`, `cookie_contains`
- **Network setup and verification**: implement `network_called` for real and add `network_mock` setup steps for deterministic flows
- **Story YAML format** (`*.story.yaml`): chains flows with preconditions and human-readable `then` assertions
- **Story discovery**: auto-detects stories from auth gates + page relationships during `rayo-ui discover`
- **Story runner**: shared browser session across flows, precondition resolution via topological sort, setup/teardown execution for suite-scoped mocks and cleanup
- **Non-developer dashboard**: Stories tab with persona badges, plain-English descriptions, no CSS selectors

## Scope

- 4 new assertion types (types.rs + runner.rs)
- `network_mock` step support for suite-scoped setup
- `network_called` assertion wired to captured requests
- Story types, loader, discovery algorithm, YAML generator
- Story runner with precondition resolution + shared browser
- CLI: `rayo-ui stories`, update `rayo-ui scan`
- Dashboard: Stories tab with askama/HTMX templates
- Persistence, narrative, report extensions

## Not in scope

- AI-powered story inference (template-based for now)
- Cross-app story testing (single app per scan)
- Story recording/replay
- Parallel story execution
