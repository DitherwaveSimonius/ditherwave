use crate::image::{ColorSpace, WorkingImage};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error(transparent)]
    Image(#[from] image::ImageError),
}

/// Decodes a raster file (PNG, JPEG, ...) into a [`WorkingImage`]. Pixels are kept
/// gamma-encoded (`ColorSpace::Srgb`), matching what classic dithering algorithms
/// (Floyd-Steinberg and friends) expect.
pub fn open_raster(path: &Path) -> Result<WorkingImage, IoError> {
    let decoded = image::open(path)?.to_rgba8();
    let (width, height) = decoded.dimensions();
    let pixels = decoded
        .into_raw()
        .into_iter()
        .map(|b| b as f32 / 255.0)
        .collect();
    Ok(WorkingImage {
        width,
        height,
        color_space: ColorSpace::Srgb,
        pixels,
    })
}

/// Encodes a [`WorkingImage`] to a raster file, format chosen by `path`'s extension.
pub fn save_raster(image: &WorkingImage, path: &Path) -> Result<(), IoError> {
    let bytes: Vec<u8> = image
        .pixels
        .iter()
        .map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8)
        .collect();
    let buf = image::RgbaImage::from_raw(image.width, image.height, bytes)
        .expect("WorkingImage pixel buffer length must match width*height*4");
    buf.save(path)?;
    Ok(())
}

/// Encodes a [`WorkingImage`] as PNG bytes in memory, for sending a live preview to
/// the UI without touching disk.
pub fn encode_png(image: &WorkingImage) -> Result<Vec<u8>, IoError> {
    let bytes: Vec<u8> = image
        .pixels
        .iter()
        .map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8)
        .collect();
    let buf = image::RgbaImage::from_raw(image.width, image.height, bytes)
        .expect("WorkingImage pixel buffer length must match width*height*4");
    let mut out = Vec::new();
    buf.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)?;
    Ok(out)
}
