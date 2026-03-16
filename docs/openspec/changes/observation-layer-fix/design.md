# Design: Observation Layer Fix

## Architecture

```
AI Agent
  │
  ├── rayo_observe(mode: "text", selector: ".card", max_elements: 50)
  │       │
  │       ▼
  │   handle_observe() → page.text_content(selector, max_elements)
  │       │
  │       ▼
  │   JS: querySelectorAll(sel) → join(\n) → cap at max_elements
  │       │
  │       ▼
  │   Empty string on 0 matches (null-safe)
  │
  ├── rayo_observe(mode: "page_map", selector: ".sidebar")
  │       │
  │       ▼
  │   handle_observe() → page.page_map(Some(selector))
  │       │
  │       ▼
  │   JS: root = querySelector(sel) || document
  │       querySelectorAll(interactive selectors) scoped to root
  │       headings + text_summary scoped to root
  │
  ├── rayo_batch(actions: [...], abort_on_failure: true)
  │       │
  │       ▼
  │   execute_batch(actions, true)
  │       │
  │       ▼
  │   On failure → mark remaining as "Skipped (abort_on_failure)" → break
  │
  └── rayo_network(mode: "capture") → navigate → rayo_network(mode: "requests")
          │
          ▼
      enable_network_interception(network: Arc<Mutex<NetworkInterceptor>>)
          │
          ▼
      CDP Fetch.enable() → spawn listener for Fetch.requestPaused
          │
          ├── should_block() → Fetch.failRequest(BlockedByClient)
          ├── find_mock() → Fetch.fulfillRequest(status, body, headers)
          └── default → record_request() + Fetch.continueRequest
```

## Key decisions
- `querySelectorAll` replaces `querySelector` as default (no backward compat needed — pre-launch)
- `max_elements` default is 50 — prevents token explosions from broad selectors
- Scoped page_map generates dynamic JS (not modifying the EXTRACT_PAGE_MAP_JS constant)
- Network interception wired per-page via `enable_network_interception()` called from server
- `abort_on_failure` defaults to `false` to preserve existing batch behavior
