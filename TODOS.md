# TODOS

## P1: Pre-launch

### Publish to crates.io + first release
- Publish rayo-profiler → rayo-rules → rayo-updater → rayo-core → rayo-mcp in dependency order
- Tag v0.1.0 to trigger cargo-dist release (pre-built binaries for macOS arm64/x86_64, Linux x86_64)
- Verify auto-update works end-to-end (install old version, tag new, check update fires)

### CDP input events for click/type
- Replace JS `el.click()` with `Input.dispatchMouseEvent` (real mouse events, handles overlays)
- Replace JS `el.value=` with `Input.dispatchKeyEvent` (real keyboard, works with React/Vue)
- chromiumoxide has `Element::click()` and `Page::type_str()` that use CDP input internally
- Fall back to JS evaluate only for scroll/select

### Event-driven waits (replace polling)
- Subscribe to CDP `DOM.childNodeInserted`/`DOM.attributeModified` events
- Resolve wait futures immediately on match instead of 50ms polling
- Keep polling as fallback for Shadow DOM / iframe edge cases
- Wire `DOM.documentUpdated` events to `SelectorCache::invalidate()`

### Wire selector cache into resolve_selector()
- Currently `SelectorCache` is built but never called from `resolve_selector()`
- Check LRU cache before page_map lookup
- Store `remote_object_id` for fast element reuse
- Invalidate on DOM mutation events (depends on event-driven waits)

## P2: Post-launch

### Accessibility tree observation mode
- Add `a11y` mode to `rayo_observe` using CDP `Accessibility.getFullAXTree`
- Even more token-efficient than page_map for complex pages
- Returns semantic structure (roles, names, states)

### Chrome health check + auto-reconnect
- Detect when Chrome process dies (health ping via `Browser.getVersion`)
- Auto-relaunch Chrome and recreate tabs on failure
- Log warning when reconnecting

### Fix silent action failures
- Check `el.readOnly || el.disabled` before type_text, return error
- Check `history.length` before back/forward (already partially done)
- Check page `document.readyState` before screenshot

### Auto-update enhancements
- Update channels (stable/beta) — let users opt into pre-release versions
- Version telemetry (opt-in, anonymous) — see fleet version distribution for rollout confidence
- Extract rayo-updater as a generic MCP tool auto-updater crate on crates.io

## P1: Visual Testing Platform (rayo-ui)

### rayo-visual crate — Rust-native image diff engine
- SIMD-accelerated pixel comparison (YIQ color space + anti-aliasing detection)
- Perceptual hash pre-filter (pHash/dHash via img_hash) for instant identical-image detection
- Structural similarity scoring (SSIM via dssim or zensim)
- Region clustering — group nearby diff pixels into named regions with bounding boxes
- Diff overlay generation — highlighted image showing changed pixels
- Baseline management — save/load/list/delete PNG baselines with metadata
- Path sanitization — reject traversal attacks, allow only `[a-zA-Z0-9_-]` names
- Dimension mismatch detection — report both sizes when baseline vs current differ
- Blank screenshot detection — hash-based detection of all-white/all-black captures
- Zero rayo deps — publishable independently on crates.io
- Criterion benchmarks for diff at various image sizes
- **Effort: M | Priority: P1 | Blocks: screenshot assertions in rayo-ui**

### rayo-ui crate — Test runner engine
- YAML test definition parser (`.rayo/tests/*.test.yaml`)
- Test format: name, viewport config, setup steps, test steps with assertions
- Step executor: navigate, click, type, select, scroll, hover, press, wait, batch
- Assertion engine:
  - `page_map_contains` — assert element exists by selector/text/role
  - `text_contains` — assert visible text on page
  - `screenshot` — capture + compare against baseline (uses rayo-visual)
  - `network` — assert API calls were made with expected params
- Result collector — per-step timing, pass/fail, error messages, page maps, screenshots
- Report generator — JSON (structured, machine-readable) + HTML (human-readable with diffs)
- Axum web server for UI communication via WebSocket (live test execution updates)
- REST API: GET /api/suites, GET /api/results, POST /api/run, WS /ws/live
- CLI binary: `rayo test run`, `rayo test ui`, `rayo test list`
- Profiler integration — VisualCapture, VisualDiff, TestStep span categories
- Abort-on-failure option per suite
- Auto-create baseline on first run with `new_baseline: true` flag
- **Effort: L | Priority: P1 | Depends on: rayo-visual, rayo-core extensions**

### rayo-core extensions for visual testing
- Configurable viewport dimensions (currently hardcoded 1280x720)
- Element bounding box extraction via `getBoundingClientRect()` in page_map
- Element-level screenshots (CDP clip parameter from bounding box)
- Animation freeze — inject CSS `* { animation: none !important; transition: none !important; }` before capture
- CSP violation detection — warn if style injection is blocked
- PNG screenshot format option (lossless, for baseline comparison)
- Cap screenshot dimensions (configurable max, default 16384x16384)
- **Effort: M | Priority: P1 | Blocks: rayo-ui**

### Web UI — Next.js + shadcn/ui + magic ui
- Next.js app in `/ui` directory of monorepo
- Dashboard: test health overview, last run summary, trend charts, speed metrics
- Suite list: all test suites with pass/fail counts, duration, history sparklines
- Test detail view: step-by-step execution with page maps, screenshots, assertions, timing
- Live runner: real-time step execution via WebSocket, progress indicators
- Diff overlay viewer: side-by-side baseline vs current with highlighted changes
- Settings panel: viewport config, diff thresholds, baseline management, CI config
- shadcn/ui components + magic ui animations + Tailwind CSS
- Responsive layout (desktop-first, usable on tablet)
- **Effort: L | Priority: P1 | Depends on: rayo-ui server API**

### rayo_visual MCP tool
- New MCP tool exposing visual testing to AI agents
- Actions: capture (screenshot + save baseline), compare (diff against baseline), baseline (list/delete/update), assert (visual assertion with structured result)
- Structured JSON response with pass/fail, diff_ratio, perceptual_score, changed_regions, timing
- Optional diff overlay image in response (base64)
- Input validation: sanitize baseline names, validate thresholds (0-1)
- **Effort: S | Priority: P1 | Depends on: rayo-visual, rayo-core extensions**

### rayo-rules extensions for visual testing
- Warn if comparing without animation freeze
- Warn if baseline is >30 days old
- Warn if diff threshold is set to 0 (too strict, will flake)
- Suggest region masking if known-dynamic selectors detected
- **Effort: S | Priority: P2**

## P2: Visual Testing Phase 2

### Cross-viewport testing
- Run same test at multiple viewports (320px, 768px, 1280px, 1920px)
- Per-viewport baselines and assertions
- Responsive regression detection

### Component-level testing
- Screenshot individual components by CSS selector
- Isolated component assertions without full page context
- Component baseline library

### CI/CD integration
- GitHub Actions workflow template
- JUnit XML report format for CI systems
- Exit codes for pass/fail
- Sharding support (--shard-index / --shard-count)
- Parallel test execution across workers

### Baseline approval workflow
- `rayo_visual assert --accept` to update baseline from current
- Batch approval for intentional design changes
- PR integration showing visual diffs

### Test suite management
- Suite-level setup/teardown phases
- Shared configuration across tests
- Test tagging and filtering
- Quarantine for flaky tests

## P3: Future

### Playwright compatibility shim
- Accept Playwright MCP tool schemas, translate to rayo calls
- Instant migration path for existing Playwright MCP users

### Cloud baseline storage
- S3/GCS backend for team baseline sharing
- Branch-aware baseline management
- Baseline versioning with git commit association

### Cross-browser visual testing
- Firefox via WebDriver BiDi
- Safari/WebKit support
- Cross-browser diff reports

### AI-powered diff triage
- Send ambiguous diffs to vision model for semantic classification
- "Is this a real bug or an AA artifact?"
- Hybrid: fast Rust diff as gate, AI for flagged diffs only

### Design system extraction
- Extract colors, fonts, spacing, components from visual tests
- Generate design system documentation
- Design drift detection

### Speed score in rayo_profile
- A-F grade based on profiler data and rule violations
- Helps AI agents self-improve their browser automation patterns

### .rayo-rules init command
- Generate a default `.rayo-rules` config with comments
- Make it easy for users to customize rules
