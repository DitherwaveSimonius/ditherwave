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
pub(crate) fn diffuse(
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

macro_rules! error_diffusion_effect {
    ($name:ident, $key:expr, $display:expr, $doc:expr, $kernel:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy)]
        pub struct $name;

        impl $name {
            pub const KEY: &'static str = $key;
            pub(crate) const KERNEL: &'static [(i32, i32, f32)] = $kernel;
        }

        impl Effect for $name {
            fn key(&self) -> &'static str {
                Self::KEY
            }

            fn display_name(&self) -> &'static str {
                $display
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
    };
}

error_diffusion_effect!(
    FloydSteinberg,
    "error_diffusion.floyd_steinberg",
    "Floyd-Steinberg",
    "The classic 1976 error-diffusion dither.",
    &[
        (1, 0, 7.0 / 16.0),
        (-1, 1, 3.0 / 16.0),
        (0, 1, 5.0 / 16.0),
        (1, 1, 1.0 / 16.0)
    ]
);

error_diffusion_effect!(
    Atkinson,
    "error_diffusion.atkinson",
    "Atkinson",
    "Bill Atkinson's dither for the original Macintosh: only diffuses 6/8 of the \
     error (vs. Floyd-Steinberg's full error), which loses some shadow/highlight \
     detail but gives noticeably higher-contrast, punchier output.",
    &[
        (1, 0, 1.0 / 8.0),
        (2, 0, 1.0 / 8.0),
        (-1, 1, 1.0 / 8.0),
        (0, 1, 1.0 / 8.0),
        (1, 1, 1.0 / 8.0),
        (0, 2, 1.0 / 8.0),
    ]
);

error_diffusion_effect!(
    JarvisJudiceNinke,
    "error_diffusion.jarvis_judice_ninke",
    "Jarvis-Judice-Ninke",
    "Spreads error across a wider 3-row, 5-column neighborhood than \
     Floyd-Steinberg, trading a softer, slightly blurrier look for fewer \
     directional artifacts.",
    &[
        (1, 0, 7.0 / 48.0),
        (2, 0, 5.0 / 48.0),
        (-2, 1, 3.0 / 48.0),
        (-1, 1, 5.0 / 48.0),
        (0, 1, 7.0 / 48.0),
        (1, 1, 5.0 / 48.0),
        (2, 1, 3.0 / 48.0),
        (-2, 2, 1.0 / 48.0),
        (-1, 2, 3.0 / 48.0),
        (0, 2, 5.0 / 48.0),
        (1, 2, 3.0 / 48.0),
        (2, 2, 1.0 / 48.0),
    ]
);

error_diffusion_effect!(
    Stucki,
    "error_diffusion.stucki",
    "Stucki",
    "A refinement of Jarvis-Judice-Ninke with sharper, less blurry output at a \
     similar computational cost.",
    &[
        (1, 0, 8.0 / 42.0),
        (2, 0, 4.0 / 42.0),
        (-2, 1, 2.0 / 42.0),
        (-1, 1, 4.0 / 42.0),
        (0, 1, 8.0 / 42.0),
        (1, 1, 4.0 / 42.0),
        (2, 1, 2.0 / 42.0),
        (-2, 2, 1.0 / 42.0),
        (-1, 2, 2.0 / 42.0),
        (0, 2, 4.0 / 42.0),
        (1, 2, 2.0 / 42.0),
        (2, 2, 1.0 / 42.0),
    ]
);

error_diffusion_effect!(
    Sierra,
    "error_diffusion.sierra",
    "Sierra",
    "Frankie Sierra's 3-row filter: similar quality to Stucki/JJN with a \
     smaller, cheaper kernel.",
    &[
        (1, 0, 5.0 / 32.0),
        (2, 0, 3.0 / 32.0),
        (-2, 1, 2.0 / 32.0),
        (-1, 1, 4.0 / 32.0),
        (0, 1, 5.0 / 32.0),
        (1, 1, 4.0 / 32.0),
        (2, 1, 2.0 / 32.0),
        (-1, 2, 2.0 / 32.0),
        (0, 2, 3.0 / 32.0),
        (1, 2, 2.0 / 32.0),
    ]
);

error_diffusion_effect!(
    SierraLite,
    "error_diffusion.sierra_lite",
    "Sierra Lite",
    "A tiny 2-pixel, 2-row Sierra variant: the cheapest error diffusion here, \
     noticeably coarser-looking than the others.",
    &[(1, 0, 2.0 / 4.0), (-1, 1, 1.0 / 4.0), (0, 1, 1.0 / 4.0)]
);

error_diffusion_effect!(
    Burkes,
    "error_diffusion.burkes",
    "Burkes",
    "A 2-row simplification of Stucki, dropping the third row for a cheaper \
     kernel at a similar quality to Sierra.",
    &[
        (1, 0, 8.0 / 32.0),
        (2, 0, 4.0 / 32.0),
        (-2, 1, 2.0 / 32.0),
        (-1, 1, 4.0 / 32.0),
        (0, 1, 8.0 / 32.0),
        (1, 1, 4.0 / 32.0),
        (2, 1, 2.0 / 32.0),
    ]
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algorithms::test_util::{no_cancel, solid_image};
    use std::collections::HashMap;

    const ALL: &[&dyn Effect] = &[
        &FloydSteinberg,
        &Atkinson,
        &JarvisJudiceNinke,
        &Stucki,
        &Sierra,
        &SierraLite,
        &Burkes,
    ];

    #[test]
    fn preserves_dimensions_and_quantizes_to_binary_levels() {
        let input = solid_image(8, 8, 0.5);
        let ctx = no_cancel();
        for effect in ALL {
            let out = effect.apply(&input, &HashMap::new(), &ctx).unwrap();
            assert_eq!((out.width, out.height), (8, 8), "effect '{}'", effect.key());
            assert!(
                out.pixels[0..3].iter().all(|&c| c == 0.0 || c == 1.0),
                "effect '{}'",
                effect.key()
            );
        }
    }

    #[test]
    fn mid_gray_dithers_to_roughly_half_on_pixels() {
        // A large enough solid mid-gray field should average out close to 50% "on"
        // pixels once error diffusion spreads the rounding error around, for every
        // kernel (not just Floyd-Steinberg).
        let input = solid_image(32, 32, 0.5);
        for effect in ALL {
            let out = effect.apply(&input, &HashMap::new(), &no_cancel()).unwrap();
            let on = (0..32 * 32).filter(|&i| out.pixels[i * 4] == 1.0).count();
            assert!(
                (350..674).contains(&on),
                "effect '{}': expected ~512/1024 on pixels, got {on}",
                effect.key()
            );
        }
    }

    #[test]
    fn kernel_weights_sum_to_expected_totals() {
        // Every kernel here except Atkinson (which intentionally under-diffuses)
        // should redistribute the full quantization error.
        let full_diffusion: &[&[(i32, i32, f32)]] = &[
            FloydSteinberg::KERNEL,
            JarvisJudiceNinke::KERNEL,
            Stucki::KERNEL,
            Sierra::KERNEL,
            SierraLite::KERNEL,
            Burkes::KERNEL,
        ];
        for kernel in full_diffusion {
            let total: f32 = kernel.iter().map(|&(_, _, w)| w).sum();
            assert!(
                (total - 1.0).abs() < 1e-5,
                "kernel sums to {total}, expected 1.0"
            );
        }

        let atkinson_total: f32 = Atkinson::KERNEL.iter().map(|&(_, _, w)| w).sum();
        assert!((atkinson_total - 0.75).abs() < 1e-6);
    }

    #[test]
    fn every_effect_has_a_unique_key() {
        let mut keys: Vec<&str> = ALL.iter().map(|e| e.key()).collect();
        let unique_count = {
            keys.sort_unstable();
            keys.dedup();
            keys.len()
        };
        assert_eq!(unique_count, ALL.len());
    }
}
