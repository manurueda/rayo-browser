# TODOS

## P1: Pre-launch

### Publish to crates.io + cargo-dist binary releases
- Publish rayo-profiler → rayo-rules → rayo-core → rayo-mcp in dependency order
- Add cargo-dist for pre-built binaries (macOS arm64/x86_64, Linux x86_64)
- Verify `cargo install rayo-mcp` works end-to-end

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

## P3: Future

### Playwright compatibility shim
- Accept Playwright MCP tool schemas, translate to rayo calls
- Instant migration path for existing Playwright MCP users

### Visual regression testing
- Screenshot diff between runs
- Report pixel differences
- Useful for QA workflows

### Speed score in rayo_profile
- A-F grade based on profiler data and rule violations
- Helps AI agents self-improve their browser automation patterns

### .rayo-rules init command
- Generate a default `.rayo-rules` config with comments
- Make it easy for users to customize rules
