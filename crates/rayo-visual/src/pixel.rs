//! YIQ color space pixel diff with anti-aliasing detection.
//!
//! Based on:
//! - Kotsarenko & Ramos 2010: "Measuring perceived color difference using YIQ NTSC
//!   transmission color space in mobile applications"
//! - Vysniauskas 2009: "Anti-aliased Pixel and Intensity Slope Detector"

/// Convert RGB to YIQ and compute perceptual color distance squared.
///
/// Returns a value in `[0.0, 1.0]` where 0 = identical, 1 = max difference.
#[inline]
fn color_delta_sq(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8) -> f64 {
    let (r1, g1, b1) = (r1 as f64, g1 as f64, b1 as f64);
    let (r2, g2, b2) = (r2 as f64, g2 as f64, b2 as f64);

    // RGB to YIQ
    let y1 = r1 * 0.29889531 + g1 * 0.58662247 + b1 * 0.11448223;
    let i1 = r1 * 0.59597799 - g1 * 0.27417610 - b1 * 0.32180189;
    let q1 = r1 * 0.21147017 - g1 * 0.52261711 + b1 * 0.31114694;

    let y2 = r2 * 0.29889531 + g2 * 0.58662247 + b2 * 0.11448223;
    let i2 = r2 * 0.59597799 - g2 * 0.27417610 - b2 * 0.32180189;
    let q2 = r2 * 0.21147017 - g2 * 0.52261711 + b2 * 0.31114694;

    let dy = y1 - y2;
    let di = i1 - i2;
    let dq = q1 - q2;

    // Weighted: Y (luminance) dominates human perception
    0.5053 * dy * dy + 0.299 * di * di + 0.1957 * dq * dq
}

/// Check if a pixel at (x, y) is likely anti-aliased by examining its 3x3 neighborhood.
/// A pixel is AA if it sits on a high-contrast edge where the intensity slope changes direction.
fn is_antialiased(img: &[u8], width: u32, height: u32, x: u32, y: u32, other_img: &[u8]) -> bool {
    let w = width as i32;
    let h = height as i32;
    let cx = x as i32;
    let cy = y as i32;

    let mut min_dist = 0.0_f64;
    let mut max_dist = 0.0_f64;
    let mut neighbors_in_same_dir = 0;
    let mut has_high_contrast_neighbor = false;

    // Get center pixel from the image
    let ci = (cy * w + cx) as usize * 4;
    let (cr, cg, cb) = (img[ci], img[ci + 1], img[ci + 2]);

    for dy in -1..=1_i32 {
        for dx in -1..=1_i32 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = cx + dx;
            let ny = cy + dy;
            if nx < 0 || nx >= w || ny < 0 || ny >= h {
                continue;
            }

            let ni = (ny * w + nx) as usize * 4;

            // Compare neighbor in same image to center
            let (nr, ng, nb) = (img[ni], img[ni + 1], img[ni + 2]);
            let dist = color_delta_sq(cr, cg, cb, nr, ng, nb);

            if dist > max_dist {
                max_dist = dist;
            }
            if dist < min_dist || (min_dist == 0.0 && max_dist > 0.0) {
                min_dist = dist;
            }

            // Check contrast: is this neighbor very different from center?
            if dist > 3000.0 {
                has_high_contrast_neighbor = true;
            }

            // Compare neighbor in other image
            let (onr, ong, onb) = (other_img[ni], other_img[ni + 1], other_img[ni + 2]);
            let other_dist = color_delta_sq(nr, ng, nb, onr, ong, onb);
            if other_dist < dist {
                neighbors_in_same_dir += 1;
            }
        }
    }

    // Pixel is AA if it's on a high-contrast edge and most neighbors
    // in the other image are closer to the same-image neighbor than the edge pixel
    has_high_contrast_neighbor && neighbors_in_same_dir >= 3
}

/// Result of pixel-level comparison.
pub struct PixelDiffResult {
    /// Number of pixels that differ (excluding AA if configured).
    pub diff_count: u32,
    /// Total comparable pixels (excludes masked).
    pub total_count: u32,
    /// Ratio of different pixels to total.
    pub diff_ratio: f64,
    /// RGBA diff image (same dimensions as input).
    /// Changed pixels are red, unchanged are dimmed.
    pub diff_image: Option<Vec<u8>>,
    /// Grid of diff density per 32x32 block (for clustering).
    pub block_diffs: Vec<Vec<u32>>,
    /// Block grid dimensions.
    pub block_cols: usize,
    pub block_rows: usize,
}

/// Configuration for pixel-level diff.
pub struct PixelDiffConfig<'a> {
    pub width: u32,
    pub height: u32,
    /// Per-pixel YIQ distance threshold (0.0-1.0).
    pub threshold: f64,
    /// Whether to count anti-aliased pixels as differences.
    pub include_aa: bool,
    /// List of (x, y, w, h) rectangles to exclude.
    pub mask_regions: &'a [(u32, u32, u32, u32)],
    /// Whether to produce the diff image.
    pub generate_overlay: bool,
}

/// Perform pixel-level diff between two RGBA buffers of the same dimensions.
pub fn pixel_diff(baseline: &[u8], current: &[u8], config: &PixelDiffConfig) -> PixelDiffResult {
    let PixelDiffConfig {
        width,
        height,
        threshold,
        include_aa,
        mask_regions,
        generate_overlay,
    } = *config;
    let pixel_count = (width * height) as usize;
    let raw_threshold = threshold * threshold * 35215.0; // max YIQ delta squared

    let block_size = 32_u32;
    let block_cols = width.div_ceil(block_size) as usize;
    let block_rows = height.div_ceil(block_size) as usize;
    let mut block_diffs = vec![vec![0u32; block_cols]; block_rows];

    let mut diff_count = 0u32;
    let mut total_count = 0u32;
    let mut overlay = if generate_overlay {
        vec![0u8; pixel_count * 4]
    } else {
        Vec::new()
    };

    for y in 0..height {
        for x in 0..width {
            let i = (y * width + x) as usize * 4;

            // Check mask
            if is_masked(x, y, mask_regions) {
                if generate_overlay {
                    // Masked pixels shown as dark gray
                    overlay[i] = 40;
                    overlay[i + 1] = 40;
                    overlay[i + 2] = 40;
                    overlay[i + 3] = 255;
                }
                continue;
            }

            total_count += 1;

            let (r1, g1, b1) = (baseline[i], baseline[i + 1], baseline[i + 2]);
            let (r2, g2, b2) = (current[i], current[i + 1], current[i + 2]);

            let delta = color_delta_sq(r1, g1, b1, r2, g2, b2);

            if delta > raw_threshold {
                // Check anti-aliasing
                if !include_aa && is_antialiased(baseline, width, height, x, y, current) {
                    // AA pixel — skip
                    if generate_overlay {
                        // Show AA pixels as yellow (dim)
                        overlay[i] = 180;
                        overlay[i + 1] = 180;
                        overlay[i + 2] = 0;
                        overlay[i + 3] = 128;
                    }
                    continue;
                }

                diff_count += 1;

                // Track in block grid
                let bx = (x / block_size) as usize;
                let by = (y / block_size) as usize;
                if by < block_rows && bx < block_cols {
                    block_diffs[by][bx] += 1;
                }

                if generate_overlay {
                    // Diff pixels in red
                    overlay[i] = 255;
                    overlay[i + 1] = 0;
                    overlay[i + 2] = 0;
                    overlay[i + 3] = 255;
                }
            } else if generate_overlay {
                // Unchanged pixels dimmed
                overlay[i] = r2 / 3;
                overlay[i + 1] = g2 / 3;
                overlay[i + 2] = b2 / 3;
                overlay[i + 3] = 255;
            }
        }
    }

    let diff_ratio = if total_count > 0 {
        diff_count as f64 / total_count as f64
    } else {
        0.0
    };

    PixelDiffResult {
        diff_count,
        total_count,
        diff_ratio,
        diff_image: if generate_overlay {
            Some(overlay)
        } else {
            None
        },
        block_diffs,
        block_cols,
        block_rows,
    }
}

#[inline]
fn is_masked(x: u32, y: u32, masks: &[(u32, u32, u32, u32)]) -> bool {
    for &(mx, my, mw, mh) in masks {
        if x >= mx && x < mx + mw && y >= my && y < my + mh {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_solid_rgba(width: u32, height: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
        let count = (width * height) as usize;
        let mut buf = Vec::with_capacity(count * 4);
        for _ in 0..count {
            buf.extend_from_slice(&[r, g, b, 255]);
        }
        buf
    }

    fn cfg(w: u32, h: u32, masks: &[(u32, u32, u32, u32)], overlay: bool) -> PixelDiffConfig<'_> {
        PixelDiffConfig {
            width: w,
            height: h,
            threshold: 0.1,
            include_aa: false,
            mask_regions: masks,
            generate_overlay: overlay,
        }
    }

    #[test]
    fn identical_images_produce_zero_diff() {
        let img = make_solid_rgba(100, 100, 128, 64, 200);
        let result = pixel_diff(&img, &img, &cfg(100, 100, &[], false));
        assert_eq!(result.diff_count, 0);
        assert_eq!(result.diff_ratio, 0.0);
    }

    #[test]
    fn completely_different_images_produce_high_diff() {
        let black = make_solid_rgba(100, 100, 0, 0, 0);
        let white = make_solid_rgba(100, 100, 255, 255, 255);
        let result = pixel_diff(&black, &white, &cfg(100, 100, &[], false));
        assert_eq!(result.diff_count, 10000);
        assert!((result.diff_ratio - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn masked_region_excludes_pixels() {
        let black = make_solid_rgba(100, 100, 0, 0, 0);
        let white = make_solid_rgba(100, 100, 255, 255, 255);
        let result = pixel_diff(&black, &white, &cfg(100, 100, &[(0, 0, 100, 100)], false));
        assert_eq!(result.diff_count, 0);
        assert_eq!(result.total_count, 0);
    }

    #[test]
    fn single_pixel_diff_detected() {
        let img1 = make_solid_rgba(10, 10, 100, 100, 100);
        let mut img2 = img1.clone();
        img2[0] = 255;
        img2[1] = 0;
        img2[2] = 0;
        let c = PixelDiffConfig {
            include_aa: true,
            ..cfg(10, 10, &[], false)
        };
        let result = pixel_diff(&img1, &img2, &c);
        assert_eq!(result.diff_count, 1);
    }

    #[test]
    fn overlay_generated_when_requested() {
        let black = make_solid_rgba(10, 10, 0, 0, 0);
        let white = make_solid_rgba(10, 10, 255, 255, 255);
        let result = pixel_diff(&black, &white, &cfg(10, 10, &[], true));
        assert!(result.diff_image.is_some());
        let overlay = result.diff_image.unwrap();
        assert_eq!(overlay.len(), 10 * 10 * 4);
        assert_eq!(overlay[0], 255); // R
        assert_eq!(overlay[1], 0); // G
        assert_eq!(overlay[2], 0); // B
    }
}
