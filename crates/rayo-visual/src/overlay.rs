use crate::error::VisualError;
use image::{ImageFormat, RgbaImage};

/// Encode an RGBA overlay buffer as PNG bytes.
pub fn encode_overlay_png(
    rgba_data: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>, VisualError> {
    let img = RgbaImage::from_raw(width, height, rgba_data.to_vec()).ok_or_else(|| {
        VisualError::ImageDecode(image::ImageError::Parameter(
            image::error::ParameterError::from_kind(
                image::error::ParameterErrorKind::DimensionMismatch,
            ),
        ))
    })?;

    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    img.write_to(&mut cursor, ImageFormat::Png)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_small_overlay() {
        let rgba = vec![255u8; 4 * 4 * 4]; // 4x4 white
        let png = encode_overlay_png(&rgba, 4, 4).unwrap();
        // PNG magic bytes
        assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }
}
