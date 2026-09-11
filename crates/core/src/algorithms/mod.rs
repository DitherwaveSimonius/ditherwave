pub mod color;
pub mod error_diffusion;
pub mod ordered;
pub mod pattern;

use crate::effect::{Effect, ParamDef, ParamKind, ParamValues};
use crate::image::WorkingImage;

/// Quantizes a single channel value in `[0, 1]` to one of `levels` evenly spaced
/// steps (`levels >= 2`), used by every algorithm in this module.
pub(crate) fn quantize_channel(value: f32, levels: u32) -> f32 {
    let levels = levels.max(2);
    let steps = (levels - 1) as f32;
    (value.clamp(0.0, 1.0) * steps).round() / steps
}

fn default_i64(kind: &ParamKind) -> i64 {
    match *kind {
        ParamKind::IntRange { default, .. } => default,
        ParamKind::FloatRange { default, .. } => default as i64,
        ParamKind::Bool { default } => default as i64,
        ParamKind::Choice { .. } => 0,
    }
}

fn default_f64(kind: &ParamKind) -> f64 {
    match *kind {
        ParamKind::FloatRange { default, .. } => default,
        ParamKind::IntRange { default, .. } => default as f64,
        ParamKind::Bool { default } => default as i64 as f64,
        ParamKind::Choice { .. } => 0.0,
    }
}

fn default_bool(kind: &ParamKind) -> bool {
    match *kind {
        ParamKind::Bool { default } => default,
        ParamKind::IntRange { default, .. } => default != 0,
        ParamKind::FloatRange { default, .. } => default != 0.0,
        ParamKind::Choice { .. } => false,
    }
}

fn default_str(kind: &ParamKind) -> &'static str {
    match *kind {
        ParamKind::Choice { default, .. } => default,
        _ => "",
    }
}

/// Reads an integer parameter, falling back to `def`'s default when absent/wrong type.
pub(crate) fn param_i64(params: &ParamValues, def: &ParamDef) -> i64 {
    params
        .get(def.key)
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| default_i64(&def.kind))
}

/// Reads a float parameter, falling back to `def`'s default when absent/wrong type.
pub(crate) fn param_f64(params: &ParamValues, def: &ParamDef) -> f64 {
    params
        .get(def.key)
        .and_then(|v| v.as_f64())
        .unwrap_or_else(|| default_f64(&def.kind))
}

/// Reads a bool parameter, falling back to `def`'s default when absent/wrong type.
pub(crate) fn param_bool(params: &ParamValues, def: &ParamDef) -> bool {
    params
        .get(def.key)
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| default_bool(&def.kind))
}

/// Reads a string/choice parameter, falling back to `def`'s default when absent/wrong type.
pub(crate) fn param_str(params: &ParamValues, def: &ParamDef) -> String {
    params
        .get(def.key)
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| default_str(&def.kind).to_owned())
}

/// Generates the classic recursive Bayer threshold matrix at `size x size`
/// (`size` must be a power of two), values `0..size*size`. This is the
/// "dispersed-dot" ordered-dithering matrix family.
pub(crate) fn bayer_matrix(size: usize) -> Vec<Vec<u32>> {
    debug_assert!(size.is_power_of_two());
    let mut matrix = vec![vec![0u32]];
    while matrix.len() < size {
        let n = matrix.len();
        let mut next = vec![vec![0u32; n * 2]; n * 2];
        for (y, row) in matrix.iter().enumerate() {
            for (x, &v) in row.iter().enumerate() {
                next[y][x] = 4 * v;
                next[y][x + n] = 4 * v + 2;
                next[y + n][x] = 4 * v + 3;
                next[y + n][x + n] = 4 * v + 1;
            }
        }
        matrix = next;
    }
    matrix
}

/// Generates a "clustered-dot" halftone-style threshold matrix at `size x
/// size`: cells are ranked by distance from the matrix center and assigned
/// increasing threshold values, so "on" pixels grow outward from a central
/// dot instead of the dispersed checkerboard pattern [`bayer_matrix`]
/// produces. Ties (equal distance) are broken by row then column for
/// determinism.
pub(crate) fn clustered_dot_matrix(size: usize) -> Vec<Vec<u32>> {
    let center2 = size as i32 - 1; // 2x scale avoids fractional centers
    let mut cells: Vec<(usize, usize, i32)> = Vec::with_capacity(size * size);
    for y in 0..size {
        for x in 0..size {
            let dx = 2 * x as i32 - center2;
            let dy = 2 * y as i32 - center2;
            cells.push((x, y, dx * dx + dy * dy));
        }
    }
    cells.sort_by_key(|&(x, y, d2)| (d2, y, x));

    let mut matrix = vec![vec![0u32; size]; size];
    for (rank, (x, y, _)) in cells.into_iter().enumerate() {
        matrix[y][x] = rank as u32;
    }
    matrix
}

/// Applies a threshold matrix (from [`bayer_matrix`] or [`clustered_dot_matrix`])
/// as ordered dithering: shared by both `ordered` (dispersed-dot) and `pattern`
/// (clustered-dot) algorithms, which differ only in which matrix they pass in.
pub(crate) fn threshold_matrix_dither(
    input: &WorkingImage,
    matrix: &[Vec<u32>],
    levels: u32,
) -> WorkingImage {
    let levels = levels.max(2);
    let steps = (levels - 1) as f32;
    let size = matrix.len();
    let mut buf = input.pixels.clone();

    for y in 0..input.height as usize {
        for x in 0..input.width as usize {
            // Centers the matrix entry in (-0.5, 0.5): one quantization step's worth
            // of perturbation, so nearby pixels round up/down differently instead of
            // banding.
            let bias = (matrix[y % size][x % size] as f32 + 0.5) / (size * size) as f32 - 0.5;
            let base = (y * input.width as usize + x) * 4;
            for c in 0..3 {
                let v = buf[base + c] * steps + bias;
                buf[base + c] = (v.round().clamp(0.0, steps)) / steps;
            }
        }
    }

    WorkingImage {
        width: input.width,
        height: input.height,
        color_space: input.color_space,
        pixels: buf,
    }
}

/// A registry constructor entry: an effect's key paired with a function that
/// builds a fresh boxed instance of it.
pub type EffectCtor = (&'static str, fn() -> Box<dyn Effect>);

/// All algorithms built into `ditheros-core`, for registering into a
/// [`Registry`](crate::registry::Registry).
pub fn builtins() -> Vec<EffectCtor> {
    vec![
        (error_diffusion::FloydSteinberg::KEY, || {
            Box::new(error_diffusion::FloydSteinberg)
        }),
        (error_diffusion::Atkinson::KEY, || {
            Box::new(error_diffusion::Atkinson)
        }),
        (error_diffusion::JarvisJudiceNinke::KEY, || {
            Box::new(error_diffusion::JarvisJudiceNinke)
        }),
        (error_diffusion::Stucki::KEY, || {
            Box::new(error_diffusion::Stucki)
        }),
        (error_diffusion::Sierra::KEY, || {
            Box::new(error_diffusion::Sierra)
        }),
        (error_diffusion::SierraLite::KEY, || {
            Box::new(error_diffusion::SierraLite)
        }),
        (error_diffusion::Burkes::KEY, || {
            Box::new(error_diffusion::Burkes)
        }),
        (ordered::Bayer2x2::KEY, || Box::new(ordered::Bayer2x2)),
        (ordered::Bayer4x4::KEY, || Box::new(ordered::Bayer4x4)),
        (ordered::Bayer8x8::KEY, || Box::new(ordered::Bayer8x8)),
        (pattern::Threshold::KEY, || Box::new(pattern::Threshold)),
        (pattern::ClusteredDot4x4::KEY, || {
            Box::new(pattern::ClusteredDot4x4)
        }),
        (pattern::ClusteredDot8x8::KEY, || {
            Box::new(pattern::ClusteredDot8x8)
        }),
        (color::PaletteMap::KEY, || Box::new(color::PaletteMap)),
    ]
}

#[cfg(test)]
pub(crate) mod test_util {
    use crate::effect::EffectContext;
    use crate::image::{ColorSpace, WorkingImage};

    /// A `w`x`h` image with every RGBA pixel set to `gray` (alpha opaque).
    pub(crate) fn solid_image(w: u32, h: u32, gray: f32) -> WorkingImage {
        let mut pixels = vec![gray; (w * h * 4) as usize];
        for a in pixels.iter_mut().skip(3).step_by(4) {
            *a = 1.0;
        }
        WorkingImage {
            width: w,
            height: h,
            color_space: ColorSpace::Srgb,
            pixels,
        }
    }

    pub(crate) fn no_cancel() -> EffectContext<'static> {
        EffectContext {
            rng_seed: 0,
            preview: false,
            cancelled: &|| false,
        }
    }
}
