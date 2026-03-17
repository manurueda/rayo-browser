use serde::{Deserialize, Serialize};

/// A rectangular region to exclude from comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl MaskRegion {
    /// Convert to (x, y, w, h) tuple for pixel diff.
    pub fn as_tuple(&self) -> (u32, u32, u32, u32) {
        (self.x, self.y, self.width, self.height)
    }

    /// Clamp region to image bounds.
    pub fn clamped(&self, image_width: u32, image_height: u32) -> Self {
        let x = self.x.min(image_width);
        let y = self.y.min(image_height);
        let width = self.width.min(image_width.saturating_sub(x));
        let height = self.height.min(image_height.saturating_sub(y));
        MaskRegion {
            x,
            y,
            width,
            height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_within_bounds() {
        let m = MaskRegion {
            x: 10,
            y: 20,
            width: 50,
            height: 30,
        };
        let c = m.clamped(100, 100);
        assert_eq!((c.x, c.y, c.width, c.height), (10, 20, 50, 30));
    }

    #[test]
    fn clamp_exceeding_bounds() {
        let m = MaskRegion {
            x: 90,
            y: 80,
            width: 50,
            height: 50,
        };
        let c = m.clamped(100, 100);
        assert_eq!(c.width, 10);
        assert_eq!(c.height, 20);
    }
}
