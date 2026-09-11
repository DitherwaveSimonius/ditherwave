use super::param_i64;
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

/// The classic 4x4 Bayer threshold matrix, values 0..16.
const BAYER_4X4: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

fn ordered_dither(input: &WorkingImage, matrix: &[[u8; 4]; 4], levels: u32) -> WorkingImage {
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

#[derive(Debug, Clone, Copy)]
pub struct Bayer4x4;

impl Bayer4x4 {
    pub const KEY: &'static str = "ordered.bayer_4x4";
}

impl Effect for Bayer4x4 {
    fn key(&self) -> &'static str {
        Self::KEY
    }

    fn display_name(&self) -> &'static str {
        "Bayer 4x4"
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
        Ok(ordered_dither(input, &BAYER_4X4, levels))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algorithms::test_util::{no_cancel, solid_image};
    use std::collections::HashMap;

    #[test]
    fn preserves_dimensions_and_quantizes_to_binary_levels() {
        let input = solid_image(5, 5, 0.5);
        let out = Bayer4x4
            .apply(&input, &HashMap::new(), &no_cancel())
            .unwrap();

        assert_eq!((out.width, out.height), (5, 5));
        assert!(out.pixels[0..3].iter().all(|&c| c == 0.0 || c == 1.0));
    }

    #[test]
    fn matrix_produces_a_mix_of_on_and_off_pixels_at_mid_gray() {
        let input = solid_image(4, 4, 0.5);
        let out = Bayer4x4
            .apply(&input, &HashMap::new(), &no_cancel())
            .unwrap();

        let on = (0..16).filter(|&i| out.pixels[i * 4] == 1.0).count();
        assert!(
            on > 0 && on < 16,
            "expected a dither pattern, got {on}/16 pixels on"
        );
    }
}
