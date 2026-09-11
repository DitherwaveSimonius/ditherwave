const GRID: usize = 32;
const SCALE: usize = 32; // 32x32 grid -> 1024x1024 output, nearest-neighbor upscaled.

// The classic 4x4 Bayer ordered-dithering threshold matrix (same one
// ditherwave-core's `ordered::Bayer4x4` effect uses internally), applied
// here directly against two named colors instead of per-channel, so the
// gradient dithers cleanly between exactly two tones instead of producing
// independent per-channel noise.
const BAYER_4X4: [[u32; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

// Background gradient endpoints (dark violet -> near-black).
const BG_TOP: [f32; 3] = [0.30, 0.14, 0.55];
const BG_BOTTOM: [f32; 3] = [0.03, 0.02, 0.08];

// Foreground waveform bar color + dark outline.
const BAR_FILL: [f32; 3] = [0.97, 0.95, 0.90];
const BAR_OUTLINE: [f32; 3] = [0.03, 0.02, 0.08];

/// Bar heights (in grid cells, from a 32-cell-tall canvas), forming a
/// symmetric "equalizer" waveform silhouette.
const BAR_HEIGHTS: &[usize] = &[4, 7, 11, 16, 20, 16, 11, 7, 4];
const BAR_WIDTH: usize = 2;
const BAR_GAP: usize = 1;

fn main() {
    // 1. Two-tone Bayer-dithered vertical gradient: at each cell, compare the
    // gradient's fraction against the matrix threshold to pick BG_TOP or
    // BG_BOTTOM outright (never blended), which is what gives a dithered
    // gradient its characteristic stippled transition band instead of a
    // smooth blend.
    let mut grid = vec![0.0f32; GRID * GRID * 3];
    for y in 0..GRID {
        let t = y as f32 / (GRID - 1) as f32;
        for x in 0..GRID {
            let threshold = (BAYER_4X4[y % 4][x % 4] as f32 + 0.5) / 16.0;
            let color = if t > threshold { BG_BOTTOM } else { BG_TOP };
            let base = (y * GRID + x) * 3;
            grid[base] = color[0];
            grid[base + 1] = color[1];
            grid[base + 2] = color[2];
        }
    }

    // 2. Composite the waveform bars on top, with a 1-cell dark outline.
    let total_bars_width = BAR_HEIGHTS.len() * BAR_WIDTH + (BAR_HEIGHTS.len() - 1) * BAR_GAP;
    let start_x = (GRID - total_bars_width) / 2;

    let mut set = |x: i32, y: i32, color: [f32; 3]| {
        if x < 0 || y < 0 || x >= GRID as i32 || y >= GRID as i32 {
            return;
        }
        let base = (y as usize * GRID + x as usize) * 3;
        grid[base] = color[0];
        grid[base + 1] = color[1];
        grid[base + 2] = color[2];
    };

    for (i, &h) in BAR_HEIGHTS.iter().enumerate() {
        let bar_x = start_x + i * (BAR_WIDTH + BAR_GAP);
        let bottom = GRID - 5; // leave a margin at the bottom
        let top = bottom.saturating_sub(h);
        for dx in 0..BAR_WIDTH {
            let x = (bar_x + dx) as i32;
            for y in (top as i32 - 1)..=(bottom as i32) {
                set(x - 1, y, BAR_OUTLINE);
                set(x + BAR_WIDTH as i32, y, BAR_OUTLINE);
            }
            set(x, top as i32 - 1, BAR_OUTLINE);
            set(x, bottom as i32, BAR_OUTLINE);
            for y in top as i32..bottom as i32 {
                set(x, y, BAR_FILL);
            }
        }
    }

    // 3. Nearest-neighbor upscale to a real icon resolution.
    let out_size = (GRID * SCALE) as u32;
    let mut buf = image::RgbaImage::new(out_size, out_size);
    for y in 0..out_size {
        for x in 0..out_size {
            let gx = x as usize / SCALE;
            let gy = y as usize / SCALE;
            let base = (gy * GRID + gx) * 3;
            let px = [
                (grid[base].clamp(0.0, 1.0) * 255.0).round() as u8,
                (grid[base + 1].clamp(0.0, 1.0) * 255.0).round() as u8,
                (grid[base + 2].clamp(0.0, 1.0) * 255.0).round() as u8,
                255,
            ];
            buf.put_pixel(x, y, image::Rgba(px));
        }
    }

    let out_path = std::env::args().nth(1).unwrap_or_else(|| "logo.png".to_string());
    buf.save(&out_path).expect("failed to save logo");
    println!("wrote {out_path} ({out_size}x{out_size})");
}
