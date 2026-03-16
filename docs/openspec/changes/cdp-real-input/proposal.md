# CDP Real Input: Replace JS interactions with CDP Input domain events

## Why
rayo-browser uses JavaScript `el.click()` and `el.value = x` for interactions. This is:
1. Slower than CDP Input domain events (extra JS parse/compile overhead)
2. Unreliable with React/Vue (frameworks ignore synthetic events)
3. Missing real mouse event sequence (mouseover → mousedown → mouseup)

rayo loses to Playwright on Form Fill (646ms vs 392ms) primarily because of this.

## Solution
Replace JS-based click/type with chromiumoxide's native Element APIs that use CDP `Input.dispatchMouseEvent` and `Input.dispatchKeyEvent`.

## Scope

### rayo-core
- `browser.rs` — `click()`: use `page.find_element(selector)` → `element.click()`
- `browser.rs` — `type_text()`: use `element.click()` to focus, then `element.type_str()` for keystrokes
- `browser.rs` — `select_option()`: keep JS approach with focus + proper change events

## Not in scope
- Batch pipelining (separate change)
- Selector cache wiring (separate change)
