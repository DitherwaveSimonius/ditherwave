use crate::image::WorkingImage;

/// An ordered set of colors an image can be quantized against, e.g. a built-in
/// retro palette or one extracted from an image via [`extract_kmeans`]. Colors
/// are sRGB-gamma-encoded `[r, g, b]` in `[0, 1]`, matching [`WorkingImage`]'s
/// default color space.
#[derive(Debug, Clone)]
pub struct Palette {
    pub name: String,
    pub colors: Vec<[f32; 3]>,
}

fn rgb(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
}

impl Palette {
    /// The classic 4-shade Game Boy (DMG) green palette.
    pub fn game_boy() -> Self {
        Self {
            name: "Game Boy".into(),
            colors: vec![
                rgb(0x0f, 0x38, 0x0f),
                rgb(0x30, 0x62, 0x30),
                rgb(0x8b, 0xac, 0x0f),
                rgb(0x9b, 0xbc, 0x0f),
            ],
        }
    }

    /// The 16-color PICO-8 fantasy-console palette.
    pub fn pico8() -> Self {
        Self {
            name: "PICO-8".into(),
            colors: vec![
                rgb(0x00, 0x00, 0x00),
                rgb(0x1d, 0x2b, 0x53),
                rgb(0x7e, 0x25, 0x53),
                rgb(0x00, 0x87, 0x51),
                rgb(0xab, 0x52, 0x36),
                rgb(0x5f, 0x57, 0x4f),
                rgb(0xc2, 0xc3, 0xc7),
                rgb(0xff, 0xf1, 0xe8),
                rgb(0xff, 0x00, 0x4d),
                rgb(0xff, 0xa3, 0x00),
                rgb(0xff, 0xec, 0x27),
                rgb(0x00, 0xe4, 0x36),
                rgb(0x29, 0xad, 0xff),
                rgb(0x83, 0x76, 0x9c),
                rgb(0xff, 0x77, 0xa8),
                rgb(0xff, 0xcc, 0xaa),
            ],
        }
    }

    /// Index of the closest color to `color` by Euclidean RGB distance.
    /// Panics if the palette is empty.
    pub fn nearest_index(&self, color: [f32; 3]) -> usize {
        self.colors
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                dist2(color, **a)
                    .partial_cmp(&dist2(color, **b))
                    .expect("colors are never NaN")
            })
            .map(|(i, _)| i)
            .expect("palette must not be empty")
    }
}

/// A tiny deterministic xorshift64 RNG, so palette extraction is reproducible
/// given the same seed without pulling in the `rand` crate for one call site.
struct Xorshift64(u64);

impl Xorshift64 {
    fn new(seed: u64) -> Self {
        // xorshift64 is undefined at seed 0; fall back to a fixed nonzero seed.
        Self(if seed == 0 { 0x9E3779B97F4A7C15 } else { seed })
    }

    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn next_index(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }
}

const MAX_SAMPLES: usize = 8192;
const KMEANS_ITERATIONS: usize = 10;

/// Extracts a `k`-color [`Palette`] from `image` via a simple, deterministic
/// k-means clustering over its pixels (subsampled for speed on large images).
/// `seed` makes repeated extraction of the same image reproducible, which
/// matters for recipes/batch runs.
pub fn extract_kmeans(image: &WorkingImage, k: usize, seed: u64) -> Palette {
    let k = k.max(1);
    let pixel_count = (image.width as usize) * (image.height as usize);
    let stride = (pixel_count / MAX_SAMPLES).max(1);

    let samples: Vec<[f32; 3]> = (0..pixel_count)
        .step_by(stride)
        .map(|i| {
            let base = i * 4;
            [
                image.pixels[base],
                image.pixels[base + 1],
                image.pixels[base + 2],
            ]
        })
        .collect();

    let mut rng = Xorshift64::new(seed);
    let k = k.min(samples.len().max(1));
    let mut centroids: Vec<[f32; 3]> = (0..k)
        .map(|_| samples[rng.next_index(samples.len())])
        .collect();

    for _ in 0..KMEANS_ITERATIONS {
        let mut sums = vec![[0.0f32; 3]; k];
        let mut counts = vec![0u32; k];

        for &sample in &samples {
            let nearest = (0..k)
                .min_by(|&a, &b| {
                    dist2(sample, centroids[a])
                        .partial_cmp(&dist2(sample, centroids[b]))
                        .unwrap()
                })
                .unwrap();
            for c in 0..3 {
                sums[nearest][c] += sample[c];
            }
            counts[nearest] += 1;
        }

        for i in 0..k {
            if counts[i] == 0 {
                // Empty cluster: reseed it from a random sample so it doesn't get
                // stuck contributing nothing to the palette.
                centroids[i] = samples[rng.next_index(samples.len())];
            } else {
                for c in 0..3 {
                    centroids[i][c] = sums[i][c] / counts[i] as f32;
                }
            }
        }
    }

    Palette {
        name: format!("Extracted ({k})"),
        colors: centroids,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::ColorSpace;

    #[test]
    fn nearest_index_picks_the_closest_color() {
        let palette = Palette {
            name: "test".into(),
            colors: vec![[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]],
        };
        assert_eq!(palette.nearest_index([0.1, 0.1, 0.1]), 0);
        assert_eq!(palette.nearest_index([0.9, 0.9, 0.9]), 1);
    }

    #[test]
    fn built_in_palettes_are_non_empty_and_in_range() {
        for palette in [Palette::game_boy(), Palette::pico8()] {
            assert!(!palette.colors.is_empty());
            for [r, g, b] in palette.colors {
                assert!((0.0..=1.0).contains(&r));
                assert!((0.0..=1.0).contains(&g));
                assert!((0.0..=1.0).contains(&b));
            }
        }
    }

    #[test]
    fn kmeans_extracts_the_requested_color_count() {
        // Two solid-color halves, so k=2 should cleanly separate into red and blue.
        let (w, h) = (4, 2);
        let mut pixels = vec![0.0f32; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let base = (y * w + x) * 4;
                if x < w / 2 {
                    pixels[base] = 1.0; // red
                } else {
                    pixels[base + 2] = 1.0; // blue
                }
                pixels[base + 3] = 1.0;
            }
        }
        let image = WorkingImage {
            width: w as u32,
            height: h as u32,
            color_space: ColorSpace::Srgb,
            pixels,
        };

        let palette = extract_kmeans(&image, 2, 42);
        assert_eq!(palette.colors.len(), 2);
        let has_red = palette.colors.iter().any(|c| c[0] > 0.9 && c[2] < 0.1);
        let has_blue = palette.colors.iter().any(|c| c[2] > 0.9 && c[0] < 0.1);
        assert!(
            has_red && has_blue,
            "expected red and blue clusters, got {:?}",
            palette.colors
        );
    }

    #[test]
    fn kmeans_is_deterministic_for_a_fixed_seed() {
        let image = WorkingImage::new(8, 8, ColorSpace::Srgb);
        let a = extract_kmeans(&image, 3, 7);
        let b = extract_kmeans(&image, 3, 7);
        assert_eq!(a.colors, b.colors);
    }
}
