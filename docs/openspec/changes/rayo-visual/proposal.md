# rayo-visual: Rust-native image diff engine

## Why
rayo-browser needs visual comparison capabilities for its E2E testing platform. Pixel-level screenshot diffs must be fast (sub-millisecond), accurate (anti-aliasing aware), and produce structured output that AI agents can reason about. No existing Rust crate combines all of: perceptual hashing pre-filter, SIMD pixel diff, SSIM scoring, region clustering, and diff overlay generation in a single ergonomic API.

## Solution
New `rayo-visual` crate with zero rayo dependencies — a pure image processing library publishable independently on crates.io. Multi-tier comparison pipeline: hash pre-filter (instant identical detection) → pixel diff (YIQ + AA detection) → perceptual score (SSIM) → region clustering → diff overlay. Baseline management with path sanitization and metadata.

## Architecture

```
  compare(baseline, current, options)
       │
       ├── 1. Hash pre-filter (img_hash pHash)
       │   └── Identical? → return Pass (0ms)
       │
       ├── 2. Dimension check
       │   └── Mismatch? → return DimensionMismatch report
       │
       ├── 3. Apply region masks (exclude dynamic content)
       │
       ├── 4. Pixel diff (YIQ color space + AA detection)
       │   └── diff_ratio < threshold? → return Pass
       │
       ├── 5. Perceptual score (SSIM on changed regions)
       │
       ├── 6. Region clustering (group nearby diff pixels)
       │
       ├── 7. Generate diff overlay image
       │
       └── 8. Return DiffReport {
               pass, diff_ratio, perceptual_score,
               changed_regions, diff_image, timing
           }
```

## Scope
- Multi-tier diff engine (hash → pixel → SSIM → cluster → overlay)
- Baseline save/load/list/delete with metadata
- Path sanitization (no traversal)
- Blank image detection
- Dimension mismatch handling
- Region masking (coordinate-based rectangles)
- Criterion benchmarks

## Not in scope
- Browser integration (that's rayo-core)
- Test runner logic (that's rayo-test)
- Selector-based masking (requires browser — that's rayo-core-visual-ext)
- Cloud storage (Phase 3)
