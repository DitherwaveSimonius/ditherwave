use super::{bayer_matrix, param_i64, threshold_matrix_dither};
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
const PARAMS: &[ParamDef] = &[LEVELS];

macro_rules! bayer_effect {
    ($name:ident, $key:expr, $display:expr, $size:expr) => {
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
                EffectCategory::Ordered
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
                let levels = param_i64(params, &LEVELS).clamp(2, 256) as u32;
                Ok(threshold_matrix_dither(input, &bayer_matrix($size), levels))
            }
        }
    };
}

bayer_effect!(Bayer2x2, "ordered.bayer_2x2", "Bayer 2x2", 2);
bayer_effect!(Bayer4x4, "ordered.bayer_4x4", "Bayer 4x4", 4);
bayer_effect!(Bayer8x8, "ordered.bayer_8x8", "Bayer 8x8", 8);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algorithms::bayer_matrix;
    use crate::algorithms::test_util::{no_cancel, solid_image};
    use std::collections::HashMap;

    #[test]
    fn bayer_4x4_matches_the_classic_reference_matrix() {
        let expected = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
        assert_eq!(bayer_matrix(4), expected.map(|row| row.to_vec()).to_vec());
    }

    #[test]
    fn every_bayer_size_covers_each_value_exactly_once() {
        for size in [2, 4, 8] {
            let matrix = bayer_matrix(size);
            let mut values: Vec<u32> = matrix.into_iter().flatten().collect();
            values.sort_unstable();
            assert_eq!(values, (0..(size * size) as u32).collect::<Vec<_>>());
        }
    }

    #[test]
    fn preserves_dimensions_and_quantizes_to_binary_levels() {
        let input = solid_image(9, 9, 0.5);
        let ctx = no_cancel();
        for effect in [
            &Bayer2x2 as &dyn Effect,
            &Bayer4x4 as &dyn Effect,
            &Bayer8x8 as &dyn Effect,
        ] {
            let out = effect.apply(&input, &HashMap::new(), &ctx).unwrap();
            assert_eq!((out.width, out.height), (9, 9));
            assert!(out.pixels[0..3].iter().all(|&c| c == 0.0 || c == 1.0));
        }
    }
}
