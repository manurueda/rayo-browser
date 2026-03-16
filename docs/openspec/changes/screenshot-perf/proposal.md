# Screenshot Performance: JPEG + viewport clip

## Why
rayo-browser screenshots take 527ms vs Playwright's 16.7ms (30x slower). The current implementation uses PNG format with no quality setting and ignores the `full_page` parameter, capturing full-page at maximum quality.

## Solution
Switch default screenshot format to JPEG at 80% quality with viewport clipping. PNG encode is 10-50x slower than JPEG for photographic content.

## Scope

### rayo-core
- `browser.rs` — `screenshot()` method
  - Switch `CaptureScreenshotFormat::Png` to `CaptureScreenshotFormat::Jpeg`
  - Set `quality: Some(80)` for JPEG
  - Wire `_full_page` parameter: when false, clip to viewport dimensions

### Boundary
- Only touches the screenshot code path
- No interaction with cache, batch, or navigation subsystems
- Observation format (base64 string) unchanged for MCP consumers

## Not in scope
- Configurable format/quality via MCP tool parameters (future change)
- WebP format support
