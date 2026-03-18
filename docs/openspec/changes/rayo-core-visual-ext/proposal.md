# rayo-core visual extensions

## Why
rayo-core currently captures screenshots (JPEG, viewport or full-page) and page maps (50 interactive elements with selectors). For visual testing, we need: configurable viewport dimensions, element bounding boxes in page maps, element-level screenshots, animation freezing, and PNG format support. These are foundational browser capabilities that rayo-ui and rayo_visual MCP tool both depend on.

## Solution
Extend rayo-core's existing browser.rs and page_map.rs with visual testing capabilities. No new modules — these are natural extensions of existing code.

## Changes

1. **Configurable viewport** — add `width`/`height` params to `RayoBrowser::new()` and `rayo_navigate goto`. Currently hardcoded 1280x720.
2. **Element bounding boxes** — extend page_map JavaScript to call `getBoundingClientRect()` per element. Add `x, y, width, height` to `InteractiveElement`.
3. **Element-level screenshots** — use CDP `Page.captureScreenshot` with `clip` parameter from bounding box.
4. **Animation freeze** — inject CSS `* { animation-duration: 0s !important; transition-duration: 0s !important; }` before screenshot capture. Remove after.
5. **PNG format** — add `format` param to `capture_screenshot()`. Visual testing uses PNG (lossless); regular observation keeps JPEG (fast).
6. **Screenshot dimension cap** — reject/cap full_page screenshots exceeding configurable max (default 16384x16384).

## Scope
- Viewport configuration
- Bounding boxes in page maps
- Element screenshots
- Animation freeze/unfreeze
- PNG screenshot format
- Dimension cap

## Not in scope
- Diff engine (rayo-visual)
- Test runner (rayo-ui)
- Mobile device emulation (Phase 2)
