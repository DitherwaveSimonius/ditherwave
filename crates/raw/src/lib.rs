//! RAW photo decoding: basic "decode to a working preview" scope only (no
//! manual exposure/white-balance/highlight-recovery controls) — see the M3
//! scope note in the project root README.md roadmap. Backed by `imagepipe`
//! (LGPL-3.0-only; see the licensing note there), which wraps `rawloader`'s
//! sensor decode with demosaicing and a default color pipeline so this crate
//! doesn't have to hand-roll either.

use ditherwave_core::{ColorSpace, WorkingImage};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum RawError {
    #[error("failed to decode RAW file: {0}")]
    Decode(String),
}

/// Common RAW file extensions (case-insensitive) this crate will attempt to
/// decode. Not exhaustive — `rawloader`'s actual format support is broader —
/// this is only what the desktop app's file picker and open-file routing
/// offer as "this looks like a RAW photo" up front.
pub const RAW_EXTENSIONS: &[&str] = &[
    "cr2", "cr3", "nef", "arw", "raf", "rw2", "orf", "dng", "pef", "srw",
];

/// Whether `path`'s extension matches a known RAW format (case-insensitive).
pub fn is_raw_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| RAW_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Decodes a RAW photo to a [`WorkingImage`] via `imagepipe`'s default
/// demosaic + white-balance + gamma pipeline, at full resolution.
pub fn open_raw(path: &Path) -> Result<WorkingImage, RawError> {
    let decoded =
        imagepipe::simple_decode_8bit(path, usize::MAX, usize::MAX).map_err(RawError::Decode)?;
    Ok(srgb_image_to_working_image(decoded))
}

fn srgb_image_to_working_image(image: imagepipe::SRGBImage) -> WorkingImage {
    let mut pixels = Vec::with_capacity(image.width * image.height * 4);
    let (rgb_chunks, _remainder) = image.data.as_chunks::<3>();
    for rgb in rgb_chunks {
        pixels.push(rgb[0] as f32 / 255.0);
        pixels.push(rgb[1] as f32 / 255.0);
        pixels.push(rgb[2] as f32 / 255.0);
        pixels.push(1.0);
    }
    WorkingImage {
        width: image.width as u32,
        height: image.height as u32,
        color_space: ColorSpace::Srgb,
        pixels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn recognizes_common_raw_extensions_case_insensitively() {
        for ext in ["cr2", "CR2", "Nef", "dng"] {
            assert!(is_raw_extension(&PathBuf::from(format!("photo.{ext}"))));
        }
        assert!(!is_raw_extension(&PathBuf::from("photo.png")));
        assert!(!is_raw_extension(&PathBuf::from("photo")));
    }

    #[test]
    fn converts_srgb_image_pixels_correctly() {
        // 2x1 image: pure red, then pure blue.
        let srgb = imagepipe::SRGBImage {
            width: 2,
            height: 1,
            data: vec![255, 0, 0, 0, 0, 255],
        };
        let working = srgb_image_to_working_image(srgb);

        assert_eq!((working.width, working.height), (2, 1));
        assert_eq!(working.pixels, vec![1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn open_raw_returns_an_error_rather_than_panicking_on_a_missing_file() {
        let result = open_raw(&PathBuf::from("/nonexistent/path/does-not-exist.cr2"));
        assert!(result.is_err());
    }

    #[test]
    fn open_raw_returns_an_error_rather_than_panicking_on_a_non_raw_file() {
        // Any existing non-RAW file should fail decode cleanly rather than panic.
        let this_file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let result = open_raw(&this_file);
        assert!(result.is_err());
    }
}
