# Design: Screenshot Performance

## Current implementation
- `browser.rs` `screenshot()` uses `CaptureScreenshotFormat::Png` with `..Default::default()`
- The `_full_page` parameter is accepted but ignored (prefixed with `_`)
- No quality setting applied

## Changes
1. Switch format to `CaptureScreenshotFormat::Jpeg`
2. Set `quality: Some(80)` — good balance of size vs fidelity
3. When `full_page` is false: set `clip` to viewport rect via JS `window.innerWidth/innerHeight`
4. When `full_page` is true: omit clip (capture entire page)

## Why JPEG over WebP
- JPEG encode is fastest for photographic content
- Universal browser/tool support
- WebP encode is slower than JPEG despite better compression
- 80% quality is the industry standard (Playwright uses same)
