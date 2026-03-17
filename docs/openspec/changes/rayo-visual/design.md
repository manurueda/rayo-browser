# rayo-visual: Design

## Crate structure

```
crates/rayo-visual/
├── Cargo.toml
├── src/
│   ├── lib.rs          — public API: compare(), DiffReport, DiffOptions
│   ├── hash.rs         — perceptual hash pre-filter (img_hash)
│   ├── pixel.rs        — YIQ pixel diff with anti-aliasing detection
│   ├── perceptual.rs   — SSIM scoring (dssim-core)
│   ├── cluster.rs      — region clustering of diff pixels
│   ├── overlay.rs      — diff overlay image generation
│   ├── baseline.rs     — baseline save/load/list/delete + metadata
│   ├── mask.rs         — region masking (coordinate rectangles)
│   └── error.rs        — VisualError enum
└── tests/
    ├── diff_tests.rs       — unit tests with fixture images
    └── fixtures/
        ├── page_a.png
        ├── page_b_minor.png    — 1 button color change
        ├── page_b_major.png    — layout shift
        ├── page_blank.png
        └── page_aa.png         — anti-aliased text
```

## Public API

```rust
pub struct DiffOptions {
    pub threshold: f64,           // 0.0-1.0, default 0.01 (1% of pixels)
    pub include_aa: bool,         // count anti-aliased pixels as diff? default false
    pub masks: Vec<MaskRegion>,   // regions to exclude
    pub generate_overlay: bool,   // produce diff image? default true
}

pub struct MaskRegion {
    pub x: u32, pub y: u32,
    pub width: u32, pub height: u32,
}

pub struct DiffReport {
    pub pass: bool,
    pub diff_ratio: f64,          // 0.0-1.0
    pub diff_pixel_count: u32,
    pub total_pixel_count: u32,
    pub perceptual_score: f64,    // SSIM, 0.0-1.0 (1.0 = identical)
    pub changed_regions: Vec<ChangedRegion>,
    pub diff_image: Option<Vec<u8>>,  // PNG bytes of overlay
    pub timing: DiffTiming,
    pub dimensions: (u32, u32),
}

pub struct ChangedRegion {
    pub x: u32, pub y: u32,
    pub width: u32, pub height: u32,
    pub diff_ratio: f64,          // local diff density
}

pub struct DiffTiming {
    pub hash_us: u64,
    pub pixel_us: u64,
    pub perceptual_us: u64,
    pub cluster_us: u64,
    pub overlay_us: u64,
    pub total_us: u64,
}

// Main entry point
pub fn compare(
    baseline: &[u8],  // PNG bytes
    current: &[u8],   // PNG bytes
    options: &DiffOptions,
) -> Result<DiffReport, VisualError>;

// Baseline management
pub struct BaselineManager {
    baselines_dir: PathBuf,
}

impl BaselineManager {
    pub fn new(baselines_dir: PathBuf) -> Self;
    pub fn save(&self, name: &str, png_bytes: &[u8]) -> Result<(), VisualError>;
    pub fn load(&self, name: &str) -> Result<Vec<u8>, VisualError>;
    pub fn list(&self) -> Result<Vec<BaselineInfo>, VisualError>;
    pub fn delete(&self, name: &str) -> Result<(), VisualError>;
    pub fn exists(&self, name: &str) -> bool;
}
```

## Dependencies

```toml
[dependencies]
image = "0.25"           # PNG decode/encode, pixel buffers
dssim-core = "3"         # SSIM perceptual scoring
img_hash = "3"           # Perceptual hashing (pHash)
thiserror = "2"          # Error types
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[dev-dependencies]
criterion = "0.5"
```

## Pixel Diff Algorithm (YIQ + Anti-Aliasing)

Based on Kotsarenko & Ramos 2010 and Vysniauskas 2009:

1. Convert both pixels from RGBA to YIQ color space
2. Compute weighted delta: `delta = 0.5053 * dY² + 0.299 * dI² + 0.1957 * dQ²`
3. If delta > threshold: check if anti-aliased
4. AA detection: examine 3x3 neighborhood, count pixels on high-contrast edges
5. If AA and `!include_aa`: skip (not counted as diff)
6. Otherwise: count as diff pixel, paint on overlay

## Region Clustering

Simple grid-based approach:
1. Divide image into 32x32 blocks
2. Count diff pixels per block
3. Merge adjacent blocks with diffs into regions
4. Return bounding box of each merged region

## Performance Targets

| Operation | 1280x720 | 1920x1080 | 3840x2160 |
|-----------|----------|-----------|-----------|
| Hash pre-filter | <0.1ms | <0.1ms | <0.2ms |
| Pixel diff | <1ms | <2ms | <8ms |
| SSIM (changed regions) | <3ms | <5ms | <15ms |
| Overlay generation | <2ms | <4ms | <10ms |
| **Total** | **<7ms** | **<12ms** | **<35ms** |
