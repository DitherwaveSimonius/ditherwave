pub mod error_diffusion;
pub mod ordered;
pub mod pattern;

use crate::effect::{Effect, ParamDef, ParamKind, ParamValues};

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
    }
}

fn default_f64(kind: &ParamKind) -> f64 {
    match *kind {
        ParamKind::FloatRange { default, .. } => default,
        ParamKind::IntRange { default, .. } => default as f64,
        ParamKind::Bool { default } => default as i64 as f64,
    }
}

fn default_bool(kind: &ParamKind) -> bool {
    match *kind {
        ParamKind::Bool { default } => default,
        ParamKind::IntRange { default, .. } => default != 0,
        ParamKind::FloatRange { default, .. } => default != 0.0,
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
        (ordered::Bayer4x4::KEY, || Box::new(ordered::Bayer4x4)),
        (pattern::Threshold::KEY, || Box::new(pattern::Threshold)),
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
