//! rayo-visual: Rust-native image diff engine for visual testing.
//!
//! Multi-tier comparison pipeline:
//! 1. Perceptual hash pre-filter (instant identical detection)
//! 2. Pixel diff (YIQ color space + anti-aliasing detection)
//! 3. Perceptual score (SSIM via dssim)
//! 4. Region clustering
//! 5. Diff overlay generation

pub mod baseline;
pub mod cluster;
pub mod error;
pub mod mask;
pub mod overlay;
pub mod perceptual;
pub mod pixel;

use cluster::ChangedRegion;
use error::VisualError;
use image::GenericImageView;
use mask::MaskRegion;
use serde::Serialize;
use std::time::Instant;

pub use baseline::{BaselineInfo, BaselineManager, BaselineMeta};

/// Options for image comparison.
#[derive(Debug, Clone)]
pub struct DiffOptions {
    /// Fraction of pixels that can differ before failing (0.0-1.0). Default: 0.01 (1%).
    pub threshold: f64,
    /// Count anti-aliased pixels as differences? Default: false.
    pub include_aa: bool,
    /// Regions to exclude from comparison.
    pub masks: Vec<MaskRegion>,
    /// Generate a diff overlay image? Default: true.
    pub generate_overlay: bool,
    /// Per-pixel color distance threshold (0.0-1.0). Default: 0.1.
    pub pixel_threshold: f64,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            threshold: 0.01,
            include_aa: false,
            masks: Vec::new(),
            generate_overlay: true,
            pixel_threshold: 0.1,
        }
    }
}

/// Timing breakdown for the diff pipeline.
#[derive(Debug, Clone, Serialize)]
pub struct DiffTiming {
    pub decode_us: u64,
    pub pixel_us: u64,
    pub perceptual_us: u64,
    pub cluster_us: u64,
    pub overlay_us: u64,
    pub total_us: u64,
}

/// Result of comparing two images.
#[derive(Debug, Clone, Serialize)]
pub struct DiffReport {
    /// Whether the comparison passed (diff within threshold).
    pub pass: bool,
    /// Fraction of pixels that differ (0.0-1.0).
    pub diff_ratio: f64,
    /// Number of pixels that differ.
    pub diff_pixel_count: u32,
    /// Total comparable pixels.
    pub total_pixel_count: u32,
    /// SSIM-based perceptual similarity (1.0 = identical, 0.0 = completely different).
    pub perceptual_score: f64,
    /// Clustered regions of change.
    pub changed_regions: Vec<ChangedRegion>,
    /// PNG bytes of diff overlay image (if requested).
    #[serde(skip)]
    pub diff_image: Option<Vec<u8>>,
    /// Whether a diff image was generated (for serialization).
    pub has_diff_image: bool,
    /// Image dimensions.
    pub dimensions: (u32, u32),
    /// Whether the current image appears blank.
    pub blank_detected: bool,
    /// Whether this is a new baseline (no prior baseline existed).
    pub new_baseline: bool,
    /// Pipeline timing.
    pub timing: DiffTiming,
}

/// Compare two PNG images and produce a structured diff report.
///
/// This is the main entry point for the diff engine. It runs the full
/// multi-tier pipeline: decode → pixel diff → SSIM → cluster → overlay.
pub fn compare(
    baseline_png: &[u8],
    current_png: &[u8],
    options: &DiffOptions,
) -> Result<DiffReport, VisualError> {
    let total_start = Instant::now();

    // 1. Decode both images
    let decode_start = Instant::now();
    let baseline_img = image::load_from_memory(baseline_png)?;
    let current_img = image::load_from_memory(current_png)?;
    let decode_us = decode_start.elapsed().as_micros() as u64;

    let (bw, bh) = baseline_img.dimensions();
    let (cw, ch) = current_img.dimensions();

    // 2. Dimension check
    if bw != cw || bh != ch {
        return Err(VisualError::DimensionMismatch {
            baseline_w: bw,
            baseline_h: bh,
            current_w: cw,
            current_h: ch,
        });
    }

    let width = bw;
    let height = bh;

    // Convert to RGBA
    let baseline_rgba = baseline_img.to_rgba8();
    let current_rgba = current_img.to_rgba8();
    let baseline_bytes = baseline_rgba.as_raw();
    let current_bytes = current_rgba.as_raw();

    // 3. Check for blank current image
    let blank_detected = is_blank(current_bytes, width, height);

    // 4. Pixel diff
    let mask_tuples: Vec<(u32, u32, u32, u32)> = options
        .masks
        .iter()
        .map(|m| m.clamped(width, height).as_tuple())
        .collect();

    let pixel_start = Instant::now();
    let pixel_result = pixel::pixel_diff(
        baseline_bytes,
        current_bytes,
        &pixel::PixelDiffConfig {
            width,
            height,
            threshold: options.pixel_threshold,
            include_aa: options.include_aa,
            mask_regions: &mask_tuples,
            generate_overlay: options.generate_overlay,
        },
    );
    let pixel_us = pixel_start.elapsed().as_micros() as u64;

    let pass = pixel_result.diff_ratio <= options.threshold;

    // 5. Perceptual score (only if there are diffs worth scoring)
    let perceptual_start = Instant::now();
    let perceptual_score = if pixel_result.diff_count > 0 {
        perceptual::perceptual_score(baseline_bytes, current_bytes, width, height)
    } else {
        1.0 // Identical
    };
    let perceptual_us = perceptual_start.elapsed().as_micros() as u64;

    // 6. Region clustering
    let cluster_start = Instant::now();
    let changed_regions = cluster::cluster_regions(
        &pixel_result.block_diffs,
        pixel_result.block_rows,
        pixel_result.block_cols,
        32,
        width,
        height,
    );
    let cluster_us = cluster_start.elapsed().as_micros() as u64;

    // 7. Diff overlay
    let overlay_start = Instant::now();
    let diff_image = if let Some(ref overlay_rgba) = pixel_result.diff_image {
        Some(overlay::encode_overlay_png(overlay_rgba, width, height)?)
    } else {
        None
    };
    let overlay_us = overlay_start.elapsed().as_micros() as u64;

    let total_us = total_start.elapsed().as_micros() as u64;

    Ok(DiffReport {
        pass,
        diff_ratio: pixel_result.diff_ratio,
        diff_pixel_count: pixel_result.diff_count,
        total_pixel_count: pixel_result.total_count,
        perceptual_score,
        changed_regions,
        has_diff_image: diff_image.is_some(),
        diff_image,
        dimensions: (width, height),
        blank_detected,
        new_baseline: false,
        timing: DiffTiming {
            decode_us,
            pixel_us,
            perceptual_us,
            cluster_us,
            overlay_us,
            total_us,
        },
    })
}

/// Check if an image is blank (all pixels are the same color).
fn is_blank(rgba: &[u8], _width: u32, _height: u32) -> bool {
    if rgba.len() < 4 {
        return true;
    }
    let (r0, g0, b0) = (rgba[0], rgba[1], rgba[2]);
    // Sample every 100th pixel for speed
    let step = 400; // 100 pixels * 4 bytes
    let mut i = step;
    while i < rgba.len() {
        if rgba[i] != r0 || rgba[i + 1] != g0 || rgba[i + 2] != b0 {
            return false;
        }
        i += step;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_solid_png(width: u32, height: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
        let mut img = image::RgbaImage::new(width, height);
        for pixel in img.pixels_mut() {
            *pixel = image::Rgba([r, g, b, 255]);
        }
        let mut buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buf);
        img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
        buf
    }

    #[test]
    fn identical_images_pass() {
        let img = make_solid_png(100, 100, 128, 64, 200);
        let report = compare(&img, &img, &DiffOptions::default()).unwrap();
        assert!(report.pass);
        assert_eq!(report.diff_ratio, 0.0);
        assert_eq!(report.diff_pixel_count, 0);
        assert!((report.perceptual_score - 1.0).abs() < 0.01);
        assert!(report.changed_regions.is_empty());
    }

    #[test]
    fn completely_different_images_fail() {
        let black = make_solid_png(50, 50, 0, 0, 0);
        let white = make_solid_png(50, 50, 255, 255, 255);
        let report = compare(&black, &white, &DiffOptions::default()).unwrap();
        assert!(!report.pass);
        assert!((report.diff_ratio - 1.0).abs() < f64::EPSILON);
        // DSSIM can return high similarity for uniform-color images
        // since there's no structural information. Just verify it's not 1.0.
        assert!(report.perceptual_score < 1.0);
    }

    #[test]
    fn dimension_mismatch_returns_error() {
        let small = make_solid_png(50, 50, 0, 0, 0);
        let large = make_solid_png(100, 100, 0, 0, 0);
        let result = compare(&small, &large, &DiffOptions::default());
        assert!(matches!(result, Err(VisualError::DimensionMismatch { .. })));
    }

    #[test]
    fn blank_image_detected() {
        let normal = make_solid_png(50, 50, 128, 64, 200);
        let blank = make_solid_png(50, 50, 255, 255, 255);
        let report = compare(&normal, &blank, &DiffOptions::default()).unwrap();
        assert!(report.blank_detected);
    }

    #[test]
    fn masked_region_excluded() {
        let black = make_solid_png(100, 100, 0, 0, 0);
        let white = make_solid_png(100, 100, 255, 255, 255);
        let opts = DiffOptions {
            masks: vec![MaskRegion {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            }],
            ..Default::default()
        };
        let report = compare(&black, &white, &opts).unwrap();
        assert!(report.pass);
        assert_eq!(report.diff_pixel_count, 0);
    }

    #[test]
    fn diff_overlay_generated() {
        let black = make_solid_png(20, 20, 0, 0, 0);
        let white = make_solid_png(20, 20, 255, 255, 255);
        let report = compare(&black, &white, &DiffOptions::default()).unwrap();
        assert!(report.diff_image.is_some());
        assert!(report.has_diff_image);
        // Verify it's valid PNG
        let overlay = report.diff_image.unwrap();
        assert_eq!(&overlay[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn timing_populated() {
        let img = make_solid_png(50, 50, 100, 100, 100);
        let report = compare(&img, &img, &DiffOptions::default()).unwrap();
        assert!(report.timing.total_us > 0);
    }
}
