# Design: CDP Real Input

## Click
Current: `evaluate("el.scrollIntoView(); el.click()")` — 1 CDP evaluate
New: `page.find_element(sel)` → `element.scroll_into_view()` → `element.click()` — uses Input.dispatchMouseEvent

chromiumoxide Element::click() internally:
1. Gets element box model (DOM.getBoxModel)
2. Calculates center coordinates
3. Dispatches Input.dispatchMouseEvent (mousePressed + mouseReleased)

## Type
Current: `evaluate("el.value = text; el.dispatchEvent(new Event('input'))")` — JS assignment
New: `page.find_element(sel)` → `element.click()` (focus) → `element.type_str(text)` — uses Input.dispatchKeyEvent per char

For clear: Use `element.click()` then evaluate select-all + delete keys before typing.

## Fallback
If `find_element` fails (element not found by selector), fall back to current JS approach for resilience.
