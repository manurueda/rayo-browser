# rayo-visual: Tasks

## Tasks

- [ ] Scaffold crate: Cargo.toml, lib.rs, error.rs, test fixtures
- [ ] Implement pixel diff engine (YIQ + AA detection) in pixel.rs
- [ ] Implement perceptual hash pre-filter in hash.rs
- [ ] Implement SSIM scoring wrapper in perceptual.rs
- [ ] Implement region clustering in cluster.rs
- [ ] Implement diff overlay generation in overlay.rs
- [ ] Implement baseline manager (save/load/list/delete + path sanitization) in baseline.rs
- [ ] Implement region masking in mask.rs
- [ ] Wire multi-tier pipeline in lib.rs compare() function
- [ ] Unit tests: identical images, minor diff, major diff, blank, AA, dimension mismatch, masking
- [ ] Criterion benchmarks at 720p, 1080p, 4K
- [ ] Add rayo-visual to workspace Cargo.toml
