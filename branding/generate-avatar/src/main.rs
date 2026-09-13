// Generates the GitHub profile avatar for the project's owner account: the
// same waveform logo mark as the app icon, on the same dithered violet-to-
// black backdrop as the demo image, but framed with a pixel-art border and a
// "founder" sparkle badge — a distinct, recognizable "this is the owner
// account" variant, not just the plain app icon re-used as a profile photo.

const S: usize = 512;

const BAYER_4X4: [[u32; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

fn set(buf: &mut [f32], x: i32, y: i32, c: [f32; 3]) {
    if x < 0 || y < 0 || x as usize >= S || y as usize >= S {
        return;
    }
    let i = (y as usize * S + x as usize) * 4;
    buf[i] = c[0];
    buf[i + 1] = c[1];
    buf[i + 2] = c[2];
    buf[i + 3] = 1.0;
}

fn dithered_gradient_pixel(x: i32, y: i32, t: f32, top: [f32; 3], bottom: [f32; 3], cell: i32) -> [f32; 3] {
    let (cx, cy) = ((x / cell) as usize, (y / cell) as usize);
    let threshold = (BAYER_4X4[cy % 4][cx % 4] as f32 + 0.5) / 16.0;
    if t > threshold {
        bottom
    } else {
        top
    }
}

/// The waveform-bar logo mark, scaled to `total_h` tall, centered at `cx`.
fn draw_logo(buf: &mut [f32], cx: i32, center_y: i32, total_h: i32, cell: i32) {
    let ratios: [i32; 9] = [4, 8, 13, 19, 24, 19, 13, 8, 4];
    let max_ratio = 24;
    let bar_w = total_h / 6;
    let gap = bar_w / 2;
    let total_w = 9 * bar_w + 8 * gap;
    let start_x = cx - total_w / 2;

    let fill_top = [0.45, 0.20, 0.85];
    let fill_bottom = [0.97, 0.95, 0.90];

    for (i, &r) in ratios.iter().enumerate() {
        let h = total_h * r / max_ratio;
        let bar_x = start_x + i as i32 * (bar_w + gap);
        let top = center_y - h / 2;
        let bottom = center_y + h / 2;
        for y in top..bottom {
            let t = (y - top) as f32 / (bottom - top).max(1) as f32;
            for x in bar_x..(bar_x + bar_w) {
                let color = dithered_gradient_pixel(x, y, t, fill_top, fill_bottom, cell);
                set(buf, x, y, color);
            }
        }
    }
}

/// A 4-point pixel "sparkle" badge — the classic 8-bit "special/shiny" marker
/// — used here to flag this as the owner/founder account's avatar rather
/// than a plain reuse of the app icon.
const SPARKLE: [&str; 9] = [
    "....#....",
    "....#....",
    "..#.#.#..",
    "...###...",
    "##.###.##",
    "...###...",
    "..#.#.#..",
    "....#....",
    "....#....",
];

fn draw_sparkle(buf: &mut [f32], cx: i32, cy: i32, cell: i32, color: [f32; 3]) {
    let h = SPARKLE.len() as i32;
    let w = SPARKLE[0].len() as i32;
    let start_x = cx - (w * cell) / 2;
    let start_y = cy - (h * cell) / 2;
    for (row, line) in SPARKLE.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            if ch == '#' {
                let px = start_x + col as i32 * cell;
                let py = start_y + row as i32 * cell;
                for dy in 0..cell {
                    for dx in 0..cell {
                        set(buf, px + dx, py + dy, color);
                    }
                }
            }
        }
    }
}

/// A pixel-art double-line frame just inside the canvas edge, marking this
/// image as a "badge" rather than a floating freestanding mark.
fn draw_frame(buf: &mut [f32], margin: i32, thickness: i32, color: [f32; 3]) {
    for t in 0..thickness {
        for x in margin..(S as i32 - margin) {
            set(buf, x, margin + t, color);
            set(buf, x, S as i32 - margin - 1 - t, color);
        }
        for y in margin..(S as i32 - margin) {
            set(buf, margin + t, y, color);
            set(buf, S as i32 - margin - 1 - t, y, color);
        }
    }
}

fn main() {
    let mut buf = vec![0.0f32; S * S * 4];

    let bg_top = [0.18, 0.09, 0.34];
    let bg_bottom = [0.03, 0.02, 0.06];
    for y in 0..S as i32 {
        let t = y as f32 / S as f32;
        for x in 0..S as i32 {
            let color = dithered_gradient_pixel(x, y, t, bg_top, bg_bottom, 4);
            set(&mut buf, x, y, color);
        }
    }

    draw_frame(&mut buf, 18, 4, [0.97, 0.95, 0.90]);
    draw_frame(&mut buf, 30, 2, [0.69, 0.57, 1.0]);

    draw_logo(&mut buf, S as i32 / 2, S as i32 / 2 + 30, 220, 5);
    draw_sparkle(&mut buf, S as i32 - 118, 118, 6, [0.96, 0.84, 0.62]);

    let mut out = image::RgbaImage::new(S as u32, S as u32);
    for y in 0..S {
        for x in 0..S {
            let i = (y * S + x) * 4;
            let px = [
                (buf[i] * 255.0).round() as u8,
                (buf[i + 1] * 255.0).round() as u8,
                (buf[i + 2] * 255.0).round() as u8,
                255,
            ];
            out.put_pixel(x as u32, y as u32, image::Rgba(px));
        }
    }

    let out_path = std::env::args().nth(1).unwrap_or_else(|| "avatar.png".to_string());
    out.save(&out_path).expect("failed to save avatar");
    println!("wrote {out_path} ({S}x{S})");
}
