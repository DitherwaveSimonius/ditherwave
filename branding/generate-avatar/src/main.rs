// Generates the GitHub profile avatar for the project's owner account: the
// same waveform logo mark as the app icon, on the same dithered violet-to-
// black backdrop as the demo image, framed with a circular pixel-art ring
// and a "founder" sparkle badge. GitHub displays avatars cropped to a
// circle, so — unlike the app icon — every design element here is kept
// inside a safe inscribed circle rather than running to the square canvas
// edges, and the frame is an actual ring (distance-based), not a square
// border that would look chopped off once cropped round.

const S: usize = 512;
const CX: i32 = (S / 2) as i32;
const CY: i32 = (S / 2) as i32;

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
/// — flagging this as the owner/founder account's avatar rather than a
/// plain reuse of the app icon.
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

/// A pixel-art ring (blocky, distance-based — not anti-aliased, so it stays
/// in the chunky pixel-art style), unlike a square frame this crops cleanly
/// when GitHub masks the avatar to a circle.
fn draw_ring(buf: &mut [f32], radius: i32, thickness: i32, color: [f32; 3]) {
    let r_out = radius;
    let r_in = radius - thickness;
    for y in (CY - r_out - 2)..(CY + r_out + 2) {
        for x in (CX - r_out - 2)..(CX + r_out + 2) {
            let dx = x - CX;
            let dy = y - CY;
            let d2 = dx * dx + dy * dy;
            if d2 <= r_out * r_out && d2 >= r_in * r_in {
                set(buf, x, y, color);
            }
        }
    }
}

fn main() {
    let mut buf = vec![0.0f32; S * S * 4];

    // Backdrop fills the full square — cropping to a circle only ever
    // removes part of a continuous gradient, which still looks intentional.
    let bg_top = [0.18, 0.09, 0.34];
    let bg_bottom = [0.03, 0.02, 0.06];
    for y in 0..S as i32 {
        let t = y as f32 / S as f32;
        for x in 0..S as i32 {
            let color = dithered_gradient_pixel(x, y, t, bg_top, bg_bottom, 4);
            set(&mut buf, x, y, color);
        }
    }

    // Everything else stays inside a safe inscribed circle (radius 226 of a
    // 256-radius crop), so nothing important is clipped by GitHub's round
    // avatar mask.
    draw_ring(&mut buf, 226, 7, [0.97, 0.95, 0.90]);
    draw_ring(&mut buf, 213, 3, [0.69, 0.57, 1.0]);

    draw_logo(&mut buf, CX, CY, 180, 4);
    draw_sparkle(&mut buf, CX + 100, CY - 100, 5, [0.96, 0.84, 0.62]);

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
