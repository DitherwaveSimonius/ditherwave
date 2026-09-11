/// Color space a [`WorkingImage`]'s pixel data is stored in. Effects declare which
/// space they operate in; the stack inserts a conversion only when it actually changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSpace {
    /// Gamma-encoded sRGB, the traditional space classic error-diffusion dithering
    /// (e.g. Floyd-Steinberg) is usually implemented against.
    Srgb,
    /// Linear light, useful for physically-based blur/convolution style effects.
    Linear,
    /// Perceptual Oklab, useful for palette matching and perceptual color distance.
    Oklab,
}

/// The common in-memory representation every effect operates on: a f32 RGBA buffer
/// tagged with the color space it's currently stored in.
#[derive(Debug, Clone)]
pub struct WorkingImage {
    pub width: u32,
    pub height: u32,
    pub color_space: ColorSpace,
    /// Row-major RGBA, four `f32` components per pixel.
    pub pixels: Vec<f32>,
}

impl WorkingImage {
    pub fn new(width: u32, height: u32, color_space: ColorSpace) -> Self {
        Self {
            width,
            height,
            color_space,
            pixels: vec![0.0; width as usize * height as usize * 4],
        }
    }
}
