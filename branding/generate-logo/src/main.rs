// A freestanding (transparent background) pixel-art waveform logo: the bars
// themselves are filled with a Bayer-dithered violet-to-cream gradient,
// instead of sitting on a solid dithered background card.

const GRID: usize = 32;
const SCALE: usize = 32; // 32x32 grid -> 1024x1024 output, nearest-neighbor upscaled.

// The classic 4x4 Bayer ordered-dithering threshold matrix (same one
// ditherwave-core's `ordered::Bayer4x4` effect uses internally), applied
// here directly against two named colors instead of per-channel, so the
// gradient dithers cleanly between exactly two tones instead of producing
// independent per-channel noise.
const BAYER_4X4: [[u32; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

// Bar fill gradient endpoints (violet top -> cream bottom).
const FILL_TOP: [f32; 3] = [0.45, 0.20, 0.85];
const FILL_BOTTOM: [f32; 3] = [0.97, 0.95, 0.90];

/// Bar heights (in grid cells, from a 32-cell-tall canvas), forming a
/// symmetric "equalizer" waveform silhouette. Individual freestanding bars —
/// no shared outline/background — so the gaps between them stay transparent.
const BAR_HEIGHTS: &[usize] = &[4, 8, 13, 19, 24, 19, 13, 8, 4];
const BAR_WIDTH: usize = 2;
const BAR_GAP: usize = 1;

#[derive(Clone, Copy, Default)]
struct Cell {
    color: [f32; 3],
    alpha: u8,
}

fn dithered_fill(x: usize, y: usize, top_of_shape: usize, bottom_of_shape: usize) -> [f32; 3] {
    let span = (bottom_of_shape - top_of_shape).max(1) as f32;
    let t = (y - top_of_shape) as f32 / span;
    let threshold = (BAYER_4X4[y % 4][x % 4] as f32 + 0.5) / 16.0;
    if t > threshold {
        FILL_BOTTOM
    } else {
        FILL_TOP
    }
}

fn main() {
    let mut grid = vec![Cell::default(); GRID * GRID];
    let mut set = |x: i32, y: i32, cell: Cell| {
        if x < 0 || y < 0 || x >= GRID as i32 || y >= GRID as i32 {
            return;
        }
        grid[y as usize * GRID + x as usize] = cell;
    };

    let total_bars_width = BAR_HEIGHTS.len() * BAR_WIDTH + (BAR_HEIGHTS.len() - 1) * BAR_GAP;
    let start_x = (GRID - total_bars_width) / 2;
    let shape_top = GRID / 2 - BAR_HEIGHTS.iter().max().copied().unwrap_or(0) / 2;
    let shape_bottom = GRID / 2 + BAR_HEIGHTS.iter().max().copied().unwrap_or(0) / 2;

    for (i, &h) in BAR_HEIGHTS.iter().enumerate() {
        let bar_x = start_x + i * (BAR_WIDTH + BAR_GAP);
        let center = GRID / 2;
        let top = center - h / 2;
        let bottom = center + h.div_ceil(2);
        for dx in 0..BAR_WIDTH {
            let x = (bar_x + dx) as i32;
            for y in top..bottom {
                let color = dithered_fill(x as usize, y, shape_top, shape_bottom);
                set(x as i32, y as i32, Cell { color, alpha: 255 });
            }
        }
    }

    // Nearest-neighbor upscale to a real icon resolution, with alpha
    // preserved so the logo sits on a transparent background.
    let out_size = (GRID * SCALE) as u32;
    let mut buf = image::RgbaImage::new(out_size, out_size);
    for y in 0..out_size {
        for x in 0..out_size {
            let gx = x as usize / SCALE;
            let gy = y as usize / SCALE;
            let cell = grid[gy * GRID + gx];
            let px = [
                (cell.color[0].clamp(0.0, 1.0) * 255.0).round() as u8,
                (cell.color[1].clamp(0.0, 1.0) * 255.0).round() as u8,
                (cell.color[2].clamp(0.0, 1.0) * 255.0).round() as u8,
                cell.alpha,
            ];
            buf.put_pixel(x, y, image::Rgba(px));
        }
    }

    let out_path = std::env::args().nth(1).unwrap_or_else(|| "logo.png".to_string());
    buf.save(&out_path).expect("failed to save logo");
    println!("wrote {out_path} ({out_size}x{out_size})");
}
