# Image Diff Engine

## ADDED Requirements

### Requirement: Multi-tier image comparison produces a structured diff report

The `compare()` function SHALL accept two PNG image buffers and options, then execute a multi-tier pipeline: perceptual hash pre-filter, pixel-level YIQ diff with anti-aliasing detection, SSIM perceptual scoring, and region clustering. It SHALL return a `DiffReport` with pass/fail, diff ratio, perceptual score, changed regions, and optional diff overlay image.

#### Scenario: Identical images pass instantly via hash pre-filter
Given two identical PNG images
When compare() is called
Then the hash pre-filter detects identical images
And the result is pass=true with diff_ratio=0.0
And pixel diff and SSIM steps are skipped
And timing.hash_us is populated but timing.pixel_us is 0

#### Scenario: Minor pixel difference below threshold passes
Given two images differing by 0.5% of pixels and threshold is 0.01
When compare() is called
Then the pixel diff detects 0.5% changed pixels
And the result is pass=true with diff_ratio=0.005

#### Scenario: Pixel difference above threshold fails with regions
Given two images differing by 5% of pixels and threshold is 0.01
When compare() is called
Then the result is pass=false with diff_ratio close to 0.05
And changed_regions contains at least one ChangedRegion with bounding box
And perceptual_score contains an SSIM value between 0.0 and 1.0

#### Scenario: Anti-aliased pixels are excluded by default
Given two images that differ only in anti-aliased edge pixels
When compare() is called with default options (include_aa=false)
Then AA pixels are detected via 3x3 neighborhood analysis
And those pixels are not counted in diff_ratio
And the result is pass=true

### Requirement: Dimension mismatch is detected and reported

When baseline and current images have different dimensions, the comparison SHALL return a DimensionMismatch error containing both dimensions rather than attempting to compare.

#### Scenario: Different dimensions produce DimensionMismatch error
Given a baseline image of 1280x720 and a current image of 1440x900
When compare() is called
Then a VisualError::DimensionMismatch is returned
And the error contains both dimensions (1280x720 and 1440x900)

### Requirement: Region masking excludes specified areas from comparison

When MaskRegion rectangles are provided in options, those pixel regions SHALL be excluded from all comparison tiers (hash, pixel, SSIM).

#### Scenario: Masked region with changes still passes
Given two images where a 100x50 region at (200,300) has changed
When compare() is called with a mask covering that region
Then the masked pixels are excluded from diff computation
And the result is pass=true

### Requirement: Diff overlay image highlights changed pixels

When `generate_overlay=true`, the diff report SHALL include a PNG image where unchanged pixels are dimmed and changed pixels are highlighted in red.

#### Scenario: Overlay image shows changed regions in red
Given two images with a visible difference
When compare() is called with generate_overlay=true
Then diff_image contains valid PNG bytes
And changed pixels in the overlay are colored red (255,0,0)
And unchanged pixels are dimmed (50% opacity original)

### Requirement: Blank image detection warns on all-white or all-black screenshots

The diff engine SHALL detect blank images (all pixels same color) and include a warning in the report.

#### Scenario: All-white current image triggers blank warning
Given a normal baseline image and an all-white current image
When compare() is called
Then the result includes a blank_detected flag
And the result is pass=false
