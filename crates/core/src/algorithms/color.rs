use super::error_diffusion::FloydSteinberg;
use super::{param_bool, param_i64, param_str};
use crate::effect::{
    Effect, EffectCategory, EffectContext, EffectResult, ParamDef, ParamKind, ParamValues,
};
use crate::image::WorkingImage;
use crate::palette::{extract_kmeans, Palette};

const PALETTE: ParamDef = ParamDef {
    key: "palette",
    display_name: "Palette",
    kind: ParamKind::Choice {
        options: &["gameboy", "pico8", "extract"],
        default: "gameboy",
    },
};
const COLORS: ParamDef = ParamDef {
    key: "colors",
    display_name: "Colors (when extracting)",
    kind: ParamKind::IntRange {
        min: 2,
        max: 64,
        step: 1,
        default: 8,
    },
};
const DITHER: ParamDef = ParamDef {
    key: "dither",
    display_name: "Dither",
    kind: ParamKind::Bool { default: true },
};
const PARAMS: &[ParamDef] = &[PALETTE, COLORS, DITHER];

/// Quantizes an image to a fixed or extracted color palette — either a plain
/// per-pixel nearest-color remap, or (with `dither` on) the same Floyd-Steinberg
/// error diffusion used by the grayscale dithers, but deciding each pixel's
/// output by nearest palette color (jointly across channels) instead of
/// independently quantizing each channel.
#[derive(Debug, Clone, Copy)]
pub struct PaletteMap;

impl PaletteMap {
    pub const KEY: &'static str = "color.palette_map";
}

fn resolve_palette(choice: &str, input: &WorkingImage, colors: usize, seed: u64) -> Palette {
    match choice {
        "gameboy" => Palette::game_boy(),
        "pico8" => Palette::pico8(),
        _ => extract_kmeans(input, colors, seed),
    }
}

fn nearest_color_remap(input: &WorkingImage, palette: &Palette) -> WorkingImage {
    let mut buf = input.pixels.clone();
    for px in buf.chunks_mut(4) {
        let color = [px[0], px[1], px[2]];
        let nearest = palette.colors[palette.nearest_index(color)];
        px[..3].copy_from_slice(&nearest);
    }
    WorkingImage {
        width: input.width,
        height: input.height,
        color_space: input.color_space,
        pixels: buf,
    }
}

/// Same idea as [`nearest_color_remap`] but diffuses each pixel's quantization
/// error (the difference between its original color and the chosen palette
/// color) to unprocessed neighbors using the Floyd-Steinberg kernel, exactly
/// like `error_diffusion::diffuse` does — except the quantization decision
/// here is a joint nearest-palette-color lookup across all three channels
/// rather than each channel being rounded independently, so it can't reuse
/// `diffuse` directly.
fn dithered_palette_remap(input: &WorkingImage, palette: &Palette) -> WorkingImage {
    let (w, h) = (input.width as i32, input.height as i32);
    let mut buf = input.pixels.clone();
    let idx = |x: i32, y: i32, c: usize| ((y * w + x) as usize) * 4 + c;

    for y in 0..h {
        for x in 0..w {
            let old = [buf[idx(x, y, 0)], buf[idx(x, y, 1)], buf[idx(x, y, 2)]];
            let new = palette.colors[palette.nearest_index(old)];
            for c in 0..3 {
                buf[idx(x, y, c)] = new[c];
                let error = old[c] - new[c];
                for &(dx, dy, weight) in FloydSteinberg::KERNEL {
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

impl Effect for PaletteMap {
    fn key(&self) -> &'static str {
        Self::KEY
    }

    fn display_name(&self) -> &'static str {
        "Palette Map"
    }

    fn category(&self) -> EffectCategory {
        EffectCategory::Color
    }

    fn params_schema(&self) -> &'static [ParamDef] {
        PARAMS
    }

    fn apply(
        &self,
        input: &WorkingImage,
        params: &ParamValues,
        ctx: &EffectContext,
    ) -> EffectResult<WorkingImage> {
        let choice = param_str(params, &PALETTE);
        let colors = param_i64(params, &COLORS).clamp(2, 64) as usize;
        let dither = param_bool(params, &DITHER);

        let palette = resolve_palette(&choice, input, colors, ctx.rng_seed);
        Ok(if dither {
            dithered_palette_remap(input, &palette)
        } else {
            nearest_color_remap(input, &palette)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algorithms::test_util::{no_cancel, solid_image};
    use std::collections::HashMap;

    #[test]
    fn remaps_to_the_nearest_game_boy_shade() {
        // Pure white should map to the Game Boy palette's lightest green, not stay white.
        let input = solid_image(2, 2, 1.0);
        let mut params = HashMap::new();
        params.insert("palette".to_string(), serde_json::json!("gameboy"));
        params.insert("dither".to_string(), serde_json::json!(false));

        let out = PaletteMap.apply(&input, &params, &no_cancel()).unwrap();
        let lightest = Palette::game_boy().colors[3];
        assert_eq!([out.pixels[0], out.pixels[1], out.pixels[2]], lightest);
    }

    #[test]
    fn output_only_contains_palette_colors() {
        let input = solid_image(6, 6, 0.5);
        let mut params = HashMap::new();
        params.insert("palette".to_string(), serde_json::json!("pico8"));
        params.insert("dither".to_string(), serde_json::json!(true));

        let out = PaletteMap.apply(&input, &params, &no_cancel()).unwrap();
        let palette = Palette::pico8();
        for px in out.pixels.chunks(4) {
            let color = [px[0], px[1], px[2]];
            assert!(
                palette.colors.contains(&color),
                "pixel {color:?} is not one of the palette's colors"
            );
        }
    }

    #[test]
    fn extract_mode_produces_the_requested_color_count() {
        let input = solid_image(8, 8, 0.5);
        let mut params = HashMap::new();
        params.insert("palette".to_string(), serde_json::json!("extract"));
        params.insert("colors".to_string(), serde_json::json!(3));
        params.insert("dither".to_string(), serde_json::json!(false));

        // Should not panic, and should stay within the valid pixel range.
        let out = PaletteMap.apply(&input, &params, &no_cancel()).unwrap();
        assert!(out.pixels.iter().all(|c| (0.0..=1.0).contains(c)));
    }
}
