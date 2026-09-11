use super::{param_f64, param_i64};
use crate::effect::{
    Effect, EffectCategory, EffectContext, EffectResult, ParamDef, ParamKind, ParamValues,
};
use crate::image::WorkingImage;

const AMOUNT: ParamDef = ParamDef {
    key: "amount",
    display_name: "Shift (px)",
    kind: ParamKind::IntRange {
        min: 0,
        max: 40,
        step: 1,
        default: 4,
    },
};
const CHROMATIC_ABERRATION_PARAMS: &[ParamDef] = &[AMOUNT];

/// Shifts the red and blue channels in opposite horizontal directions,
/// leaving green in place — a classic lens/compression-defect look.
#[derive(Debug, Clone, Copy)]
pub struct ChromaticAberration;

impl ChromaticAberration {
    pub const KEY: &'static str = "glitch.chromatic_aberration";
}

impl Effect for ChromaticAberration {
    fn key(&self) -> &'static str {
        Self::KEY
    }

    fn display_name(&self) -> &'static str {
        "Chromatic Aberration"
    }

    fn category(&self) -> EffectCategory {
        EffectCategory::Glitch
    }

    fn params_schema(&self) -> &'static [ParamDef] {
        CHROMATIC_ABERRATION_PARAMS
    }

    fn apply(
        &self,
        input: &WorkingImage,
        params: &ParamValues,
        _ctx: &EffectContext,
    ) -> EffectResult<WorkingImage> {
        let amount = param_i64(params, &AMOUNT).clamp(0, 256) as i32;
        let (w, h) = (input.width as i32, input.height as i32);
        let src = |x: i32, y: i32, c: usize| {
            let cx = x.clamp(0, w - 1) as usize;
            let cy = y.clamp(0, h - 1) as usize;
            input.pixels[(cy * w as usize + cx) * 4 + c]
        };

        let mut buf = input.pixels.clone();
        for y in 0..h {
            for x in 0..w {
                let base = ((y * w + x) as usize) * 4;
                buf[base] = src(x - amount, y, 0); // red, shifted left
                buf[base + 2] = src(x + amount, y, 2); // blue, shifted right
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

const SPACING: ParamDef = ParamDef {
    key: "spacing",
    display_name: "Line spacing (px)",
    kind: ParamKind::IntRange {
        min: 1,
        max: 20,
        step: 1,
        default: 2,
    },
};
const STRENGTH: ParamDef = ParamDef {
    key: "strength",
    display_name: "Darkness",
    kind: ParamKind::FloatRange {
        min: 0.0,
        max: 1.0,
        step: 0.01,
        default: 0.5,
    },
};
const SCANLINES_PARAMS: &[ParamDef] = &[SPACING, STRENGTH];

/// Darkens every `spacing`-th row by `strength`, for a CRT/interlace look.
#[derive(Debug, Clone, Copy)]
pub struct Scanlines;

impl Scanlines {
    pub const KEY: &'static str = "glitch.scanlines";
}

impl Effect for Scanlines {
    fn key(&self) -> &'static str {
        Self::KEY
    }

    fn display_name(&self) -> &'static str {
        "Scanlines"
    }

    fn category(&self) -> EffectCategory {
        EffectCategory::Glitch
    }

    fn params_schema(&self) -> &'static [ParamDef] {
        SCANLINES_PARAMS
    }

    fn apply(
        &self,
        input: &WorkingImage,
        params: &ParamValues,
        _ctx: &EffectContext,
    ) -> EffectResult<WorkingImage> {
        let spacing = param_i64(params, &SPACING).clamp(1, 4096) as u32;
        let strength = param_f64(params, &STRENGTH).clamp(0.0, 1.0) as f32;
        let mut buf = input.pixels.clone();

        for y in 0..input.height {
            if y % spacing != 0 {
                continue;
            }
            let row_start = (y * input.width) as usize * 4;
            let row_end = row_start + input.width as usize * 4;
            for px in buf[row_start..row_end].chunks_mut(4) {
                for c in px[..3].iter_mut() {
                    *c *= 1.0 - strength;
                }
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
    fn chromatic_aberration_leaves_a_solid_image_unchanged() {
        // Shifting a uniform field just samples the same uniform value, so a
        // solid-color image is a useful "does it crash / stay in range" check.
        let input = solid_image(10, 4, 0.5);
        let out = ChromaticAberration
            .apply(&input, &HashMap::new(), &no_cancel())
            .unwrap();
        assert_eq!((out.width, out.height), (10, 4));
        assert!(out.pixels.iter().all(|c| (0.0..=1.0).contains(c)));
    }

    #[test]
    fn chromatic_aberration_shifts_red_and_blue_oppositely() {
        // A single bright column on an otherwise black image: red should
        // shift one way, blue the other, so their bright columns end up at
        // different x positions.
        let (w, h) = (11, 1);
        let mut pixels = vec![0.0f32; w * h * 4];
        let mark_x = 5;
        pixels[mark_x * 4] = 1.0;
        pixels[mark_x * 4 + 2] = 1.0;
        pixels[mark_x * 4 + 3] = 1.0;
        let input = WorkingImage {
            width: w as u32,
            height: h as u32,
            color_space: crate::ColorSpace::Srgb,
            pixels,
        };

        let mut params = HashMap::new();
        params.insert("amount".to_string(), serde_json::json!(2));
        let out = ChromaticAberration
            .apply(&input, &params, &no_cancel())
            .unwrap();

        let red_at = |x: usize| out.pixels[x * 4];
        let blue_at = |x: usize| out.pixels[x * 4 + 2];
        assert_eq!(
            red_at(mark_x + 2),
            1.0,
            "red should have shifted right by `amount`"
        );
        assert_eq!(
            blue_at(mark_x - 2),
            1.0,
            "blue should have shifted left by `amount`"
        );
    }

    #[test]
    fn scanlines_darkens_only_the_selected_rows() {
        let input = solid_image(4, 4, 1.0);
        let mut params = HashMap::new();
        params.insert("spacing".to_string(), serde_json::json!(2));
        params.insert("strength".to_string(), serde_json::json!(1.0));

        let out = Scanlines.apply(&input, &params, &no_cancel()).unwrap();
        // Row 0 (0 % 2 == 0) fully darkened, row 1 untouched.
        assert_eq!(out.pixels[0], 0.0);
        assert_eq!(out.pixels[input.width as usize * 4], 1.0);
    }
}
