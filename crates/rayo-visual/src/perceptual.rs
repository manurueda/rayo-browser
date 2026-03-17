use dssim_core::{Dssim, DssimImage};
use image::RgbaImage;

/// Compute SSIM-based perceptual similarity score between two RGBA buffers.
///
/// Returns a score where 0.0 = identical and higher = more different.
/// We convert to a 0.0-1.0 similarity scale where 1.0 = identical.
pub fn perceptual_score(baseline: &[u8], current: &[u8], width: u32, height: u32) -> f64 {
    let attr = Dssim::new();

    let baseline_img = match rgba_to_dssim(&attr, baseline, width, height) {
        Some(img) => img,
        None => return 0.0,
    };

    let current_img = match rgba_to_dssim(&attr, current, width, height) {
        Some(img) => img,
        None => return 0.0,
    };

    let (dssim_val, _) = attr.compare(&baseline_img, current_img);

    // dssim returns 0.0 for identical, higher for different.
    // Convert to similarity: 1.0 = identical, 0.0 = completely different.
    let dssim_f64: f64 = dssim_val.into();
    (1.0 - dssim_f64).max(0.0)
}

fn rgba_to_dssim(attr: &Dssim, rgba: &[u8], width: u32, height: u32) -> Option<DssimImage<f32>> {
    let img = RgbaImage::from_raw(width, height, rgba.to_vec())?;
    // dssim expects RGBA<u8> (from the rgb crate)
    let rgb_pixels: Vec<rgb::RGBA<u8>> = img
        .pixels()
        .map(|p| rgb::RGBA::new(p[0], p[1], p[2], p[3]))
        .collect();
    attr.create_image_rgba(&rgb_pixels, width as usize, height as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_solid(width: u32, height: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
        let n = (width * height) as usize;
        let mut buf = Vec::with_capacity(n * 4);
        for _ in 0..n {
            buf.extend_from_slice(&[r, g, b, 255]);
        }
        buf
    }

    #[test]
    fn identical_images_score_one() {
        let img = make_solid(64, 64, 128, 64, 200);
        let score = perceptual_score(&img, &img, 64, 64);
        assert!((score - 1.0).abs() < 0.001, "score was {score}");
    }

    #[test]
    fn different_images_score_low() {
        let black = make_solid(64, 64, 0, 0, 0);
        let white = make_solid(64, 64, 255, 255, 255);
        let score = perceptual_score(&black, &white, 64, 64);
        assert!(score < 0.5, "score was {score}");
    }
}
