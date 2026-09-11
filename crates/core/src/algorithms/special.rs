use super::{param_f64, param_i64};
use crate::effect::{
    Effect, EffectCategory, EffectContext, EffectResult, ParamDef, ParamKind, ParamValues,
};
use crate::image::WorkingImage;

const RADIUS: ParamDef = ParamDef {
    key: "radius",
    display_name: "Glow radius (px)",
    kind: ParamKind::IntRange {
        min: 1,
        max: 40,
        step: 1,
        default: 6,
    },
};
const STRENGTH: ParamDef = ParamDef {
    key: "strength",
    display_name: "Glow strength",
    kind: ParamKind::FloatRange {
        min: 0.0,
        max: 3.0,
        step: 0.05,
        default: 1.0,
    },
};
const PARAMS: &[ParamDef] = &[RADIUS, STRENGTH];

/// A simple separable box blur over the RGB channels, alpha untouched.
fn box_blur_rgb(input: &WorkingImage, radius: i32) -> Vec<f32> {
    let (w, h) = (input.width as i32, input.height as i32);
    let get = |buf: &[f32], x: i32, y: i32, c: usize| {
        let cx = x.clamp(0, w - 1) as usize;
        let cy = y.clamp(0, h - 1) as usize;
        buf[(cy * w as usize + cx) * 4 + c]
    };

    // Horizontal pass.
    let mut horizontal = input.pixels.clone();
    for y in 0..h {
        for x in 0..w {
            for c in 0..3 {
                let mut sum = 0.0;
                for dx in -radius..=radius {
                    sum += get(&input.pixels, x + dx, y, c);
                }
                horizontal[((y * w + x) as usize) * 4 + c] = sum / (2 * radius + 1) as f32;
            }
        }
    }

    // Vertical pass.
    let mut blurred = horizontal.clone();
    for y in 0..h {
        for x in 0..w {
            for c in 0..3 {
                let mut sum = 0.0;
                for dy in -radius..=radius {
                    sum += get(&horizontal, x, y + dy, c);
                }
                blurred[((y * w + x) as usize) * 4 + c] = sum / (2 * radius + 1) as f32;
            }
        }
    }

    blurred
}

/// A soft bloom/halo around bright areas: blurs the image, then adds back
/// only the parts where the blur is *brighter* than the original (so dark
/// areas don't get muddied), scaled by `strength`. Named after Dither Boy's
/// bespoke "Epsilon Glow" effect that inspired this project, though this is
/// an independent implementation, not a port of it.
#[derive(Debug, Clone, Copy)]
pub struct EpsilonGlow;

impl EpsilonGlow {
    pub const KEY: &'static str = "special.epsilon_glow";
}

impl Effect for EpsilonGlow {
    fn key(&self) -> &'static str {
        Self::KEY
    }

    fn display_name(&self) -> &'static str {
        "Epsilon Glow"
    }

    fn category(&self) -> EffectCategory {
        EffectCategory::Special
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
        let radius = param_i64(params, &RADIUS).clamp(1, 256) as i32;
        let strength = param_f64(params, &STRENGTH).clamp(0.0, 8.0) as f32;

        let blurred = box_blur_rgb(input, radius);
        let mut buf = input.pixels.clone();
        for (i, c) in buf.iter_mut().enumerate() {
            if i % 4 == 3 {
                continue; // alpha untouched
            }
            let glow = (blurred[i] - *c).max(0.0) * strength;
            *c = (*c + glow).clamp(0.0, 1.0);
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
    fn leaves_a_solid_image_unchanged() {
        // No brighter neighbors to glow from, so a flat field is a no-op
        // modulo float rounding.
        let input = solid_image(8, 8, 0.4);
        let out = EpsilonGlow
            .apply(&input, &HashMap::new(), &no_cancel())
            .unwrap();
        for (a, b) in out.pixels.iter().zip(input.pixels.iter()) {
            assert!((a - b).abs() < 1e-5);
        }
    }

    #[test]
    fn brightens_dark_pixels_next_to_a_bright_spot() {
        let (w, h) = (9, 9);
        let mut pixels = vec![0.0f32; w * h * 4];
        for a in pixels.iter_mut().skip(3).step_by(4) {
            *a = 1.0;
        }
        let center = (h / 2 * w + w / 2) * 4;
        pixels[center] = 1.0;
        pixels[center + 1] = 1.0;
        pixels[center + 2] = 1.0;
        let input = WorkingImage {
            width: w as u32,
            height: h as u32,
            color_space: crate::ColorSpace::Srgb,
            pixels,
        };

        let mut params = HashMap::new();
        params.insert("radius".to_string(), serde_json::json!(3));
        params.insert("strength".to_string(), serde_json::json!(2.0));
        let out = EpsilonGlow.apply(&input, &params, &no_cancel()).unwrap();

        // A neighbor of the bright spot should have picked up some glow.
        let neighbor = (h / 2 * w + w / 2 + 1) * 4;
        assert!(out.pixels[neighbor] > input.pixels[neighbor]);
    }
}
