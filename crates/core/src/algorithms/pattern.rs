use super::{clustered_dot_matrix, param_f64, param_i64, threshold_matrix_dither};
use crate::effect::{
    Effect, EffectCategory, EffectContext, EffectResult, ParamDef, ParamKind, ParamValues,
};
use crate::image::WorkingImage;

const LEVELS: ParamDef = ParamDef {
    key: "levels",
    display_name: "Levels per channel",
    kind: ParamKind::IntRange {
        min: 2,
        max: 16,
        step: 1,
        default: 2,
    },
};
const HALFTONE_PARAMS: &[ParamDef] = &[LEVELS];

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

macro_rules! clustered_dot_effect {
    ($name:ident, $key:expr, $display:expr, $size:expr) => {
        /// A "clustered-dot" halftone screen: unlike Bayer's dispersed checkerboard,
        /// "on" pixels grow outward from a central dot per cell — closer to a
        /// classic newspaper halftone print screen than to a Bayer dither.
        #[derive(Debug, Clone, Copy)]
        pub struct $name;

        impl $name {
            pub const KEY: &'static str = $key;
        }

        impl Effect for $name {
            fn key(&self) -> &'static str {
                Self::KEY
            }

            fn display_name(&self) -> &'static str {
                $display
            }

            fn category(&self) -> EffectCategory {
                EffectCategory::Pattern
            }

            fn params_schema(&self) -> &'static [ParamDef] {
                HALFTONE_PARAMS
            }

            fn apply(
                &self,
                input: &WorkingImage,
                params: &ParamValues,
                _ctx: &EffectContext,
            ) -> EffectResult<WorkingImage> {
                let levels = param_i64(params, &LEVELS).clamp(2, 256) as u32;
                Ok(threshold_matrix_dither(
                    input,
                    &clustered_dot_matrix($size),
                    levels,
                ))
            }
        }
    };
}

clustered_dot_effect!(
    ClusteredDot4x4,
    "pattern.clustered_dot_4x4",
    "Clustered Dot 4x4",
    4
);
clustered_dot_effect!(
    ClusteredDot8x8,
    "pattern.clustered_dot_8x8",
    "Clustered Dot 8x8",
    8
);

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

    #[test]
    fn clustered_dot_matrix_covers_each_value_exactly_once() {
        for size in [4, 8] {
            let matrix = super::super::clustered_dot_matrix(size);
            let mut values: Vec<u32> = matrix.into_iter().flatten().collect();
            values.sort_unstable();
            assert_eq!(values, (0..(size * size) as u32).collect::<Vec<_>>());
        }
    }

    #[test]
    fn clustered_dot_grows_from_the_center() {
        // The center cell(s) must be among the lowest-ranked (earliest "on"),
        // which is what distinguishes a clustered-dot screen from Bayer's
        // dispersed pattern.
        let matrix = super::super::clustered_dot_matrix(4);
        let center_rank = matrix[1][1]
            .min(matrix[1][2])
            .min(matrix[2][1])
            .min(matrix[2][2]);
        let corner_rank = matrix[0][0]
            .max(matrix[0][3])
            .max(matrix[3][0])
            .max(matrix[3][3]);
        assert!(center_rank < corner_rank);
    }

    #[test]
    fn halftone_effects_preserve_dimensions_and_binarize() {
        let input = solid_image(9, 9, 0.5);
        let ctx = no_cancel();
        for effect in [
            &ClusteredDot4x4 as &dyn Effect,
            &ClusteredDot8x8 as &dyn Effect,
        ] {
            let out = effect.apply(&input, &HashMap::new(), &ctx).unwrap();
            assert_eq!((out.width, out.height), (9, 9));
            assert!(out.pixels[0..3].iter().all(|&c| c == 0.0 || c == 1.0));
        }
    }
}
