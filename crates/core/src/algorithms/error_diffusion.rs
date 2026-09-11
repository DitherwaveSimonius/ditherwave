use super::{param_bool, param_i64, quantize_channel};
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
const SERPENTINE: ParamDef = ParamDef {
    key: "serpentine",
    display_name: "Serpentine scan",
    kind: ParamKind::Bool { default: true },
};
const PARAMS: &[ParamDef] = &[LEVELS, SERPENTINE];

/// Diffuses the quantization error of one pixel to its as-yet-unprocessed
/// neighbors. `(dx, dy, weight)` offsets are relative to the current pixel in
/// left-to-right scan order; `weight`s should sum to 1.0 (Atkinson deliberately
/// diffuses less than that, which is what gives it more contrast).
fn diffuse(
    input: &WorkingImage,
    kernel: &[(i32, i32, f32)],
    levels: u32,
    serpentine: bool,
) -> WorkingImage {
    let (w, h) = (input.width as i32, input.height as i32);
    let mut buf = input.pixels.clone();
    let idx = |x: i32, y: i32, c: usize| ((y * w + x) as usize) * 4 + c;

    for y in 0..h {
        let reverse = serpentine && y % 2 == 1;
        let xs: Box<dyn Iterator<Item = i32>> = if reverse {
            Box::new((0..w).rev())
        } else {
            Box::new(0..w)
        };
        for x in xs {
            for c in 0..3 {
                let old = buf[idx(x, y, c)];
                let new = quantize_channel(old, levels);
                buf[idx(x, y, c)] = new;
                let error = old - new;
                for &(dx, dy, weight) in kernel {
                    let dx = if reverse { -dx } else { dx };
                    let (nx, ny) = (x + dx, y + dy);
                    if nx >= 0 && nx < w && ny >= 0 && ny < h {
                        buf[idx(nx, ny, c)] += error * weight;
                    }
                }
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
pub struct FloydSteinberg;

impl FloydSteinberg {
    pub const KEY: &'static str = "error_diffusion.floyd_steinberg";
    const KERNEL: &'static [(i32, i32, f32)] = &[
        (1, 0, 7.0 / 16.0),
        (-1, 1, 3.0 / 16.0),
        (0, 1, 5.0 / 16.0),
        (1, 1, 1.0 / 16.0),
    ];
}

impl Effect for FloydSteinberg {
    fn key(&self) -> &'static str {
        Self::KEY
    }

    fn display_name(&self) -> &'static str {
        "Floyd-Steinberg"
    }

    fn category(&self) -> EffectCategory {
        EffectCategory::ErrorDiffusion
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
        let serpentine = param_bool(params, &SERPENTINE);
        Ok(diffuse(input, Self::KERNEL, levels, serpentine))
    }
}

/// Bill Atkinson's dither for the original Macintosh: only diffuses 6/8 of the
/// error (vs. Floyd-Steinberg's full error), which loses some shadow/highlight
/// detail but gives noticeably higher-contrast, punchier output.
#[derive(Debug, Clone, Copy)]
pub struct Atkinson;

impl Atkinson {
    pub const KEY: &'static str = "error_diffusion.atkinson";
    const KERNEL: &'static [(i32, i32, f32)] = &[
        (1, 0, 1.0 / 8.0),
        (2, 0, 1.0 / 8.0),
        (-1, 1, 1.0 / 8.0),
        (0, 1, 1.0 / 8.0),
        (1, 1, 1.0 / 8.0),
        (0, 2, 1.0 / 8.0),
    ];
}

impl Effect for Atkinson {
    fn key(&self) -> &'static str {
        Self::KEY
    }

    fn display_name(&self) -> &'static str {
        "Atkinson"
    }

    fn category(&self) -> EffectCategory {
        EffectCategory::ErrorDiffusion
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
        let serpentine = param_bool(params, &SERPENTINE);
        Ok(diffuse(input, Self::KERNEL, levels, serpentine))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algorithms::test_util::{no_cancel, solid_image};
    use std::collections::HashMap;

    #[test]
    fn preserves_dimensions_and_quantizes_to_binary_levels() {
        let input = solid_image(8, 8, 0.5);
        let ctx = no_cancel();
        for effect in [&FloydSteinberg as &dyn Effect, &Atkinson as &dyn Effect] {
            let out = effect.apply(&input, &HashMap::new(), &ctx).unwrap();
            assert_eq!((out.width, out.height), (8, 8));
            assert!(out.pixels[0..3].iter().all(|&c| c == 0.0 || c == 1.0));
        }
    }

    #[test]
    fn mid_gray_dithers_to_roughly_half_on_pixels() {
        // A large enough solid mid-gray field should average out close to 50% "on"
        // pixels once error diffusion spreads the rounding error around.
        let input = solid_image(32, 32, 0.5);
        let out = FloydSteinberg
            .apply(&input, &HashMap::new(), &no_cancel())
            .unwrap();

        let on = (0..32 * 32).filter(|&i| out.pixels[i * 4] == 1.0).count();
        assert!(
            (400..624).contains(&on),
            "expected ~512/1024 on pixels, got {on}"
        );
    }

    #[test]
    fn atkinson_diffuses_less_error_than_floyd_steinberg() {
        // Atkinson only propagates 6/8 of the error; summing the kernel weights is
        // a direct way to pin that down without depending on dither output stats.
        let fs_total: f32 = FloydSteinberg::KERNEL.iter().map(|&(_, _, w)| w).sum();
        let atkinson_total: f32 = Atkinson::KERNEL.iter().map(|&(_, _, w)| w).sum();
        assert!((fs_total - 1.0).abs() < 1e-6);
        assert!((atkinson_total - 0.75).abs() < 1e-6);
    }
}
