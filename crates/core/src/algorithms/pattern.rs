use super::param_f64;
use crate::effect::{
    Effect, EffectCategory, EffectContext, EffectResult, ParamDef, ParamKind, ParamValues,
};
use crate::image::WorkingImage;

const THRESHOLD: ParamDef = ParamDef {
    key: "threshold",
    display_name: "Threshold",
    kind: ParamKind::FloatRange {
        min: 0.0,
        max: 1.0,
        step: 0.01,
        default: 0.5,
    },
};
const PARAMS: &[ParamDef] = &[THRESHOLD];

/// The simplest possible baseline: no error diffusion, no matrix — every channel is
/// binarized against one global cutoff. Useful for comparison against the "real"
/// dithers, and as the first algorithm to wire the pipeline up against.
#[derive(Debug, Clone, Copy)]
pub struct Threshold;

impl Threshold {
    pub const KEY: &'static str = "pattern.threshold";
}

impl Effect for Threshold {
    fn key(&self) -> &'static str {
        Self::KEY
    }

    fn display_name(&self) -> &'static str {
        "Threshold"
    }

    fn category(&self) -> EffectCategory {
        EffectCategory::Pattern
    }

    fn params_schema(&self) -> &'static [ParamDef] {
        PARAMS
    }

    fn apply(
        &self,
        input: &WorkingImage,
        params: &ParamValues,
        _ctx: &EffectContext,
    ) -> EffectResult<WorkingImage> {
        let threshold = param_f64(params, &THRESHOLD).clamp(0.0, 1.0) as f32;
        let mut buf = input.pixels.clone();
        for px in buf.chunks_mut(4) {
            for c in px[..3].iter_mut() {
                *c = if *c >= threshold { 1.0 } else { 0.0 };
            }
        }
        Ok(WorkingImage {
            width: input.width,
            height: input.height,
            color_space: input.color_space,
            pixels: buf,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algorithms::test_util::{no_cancel, solid_image};
    use std::collections::HashMap;

    #[test]
    fn binarizes_around_default_threshold() {
        let dark = solid_image(2, 2, 0.2);
        let bright = solid_image(2, 2, 0.8);
        let params = HashMap::new();
        let ctx = no_cancel();

        let dark_out = Threshold.apply(&dark, &params, &ctx).unwrap();
        let bright_out = Threshold.apply(&bright, &params, &ctx).unwrap();

        assert!(dark_out.pixels[0..3].iter().all(|&c| c == 0.0));
        assert!(bright_out.pixels[0..3].iter().all(|&c| c == 1.0));
        // alpha is left untouched
        assert_eq!(dark_out.pixels[3], 1.0);
    }
}
