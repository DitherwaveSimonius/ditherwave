// Generates the app's default "demo" image: the Ditherwave logo mark and
// wordmark, large and centered, in front of a simple Bayer-dithered
// violet-to-black backdrop that stays deliberately quiet — the point is the
// brand, not the background. Built with the same techniques the real
// dithering engine uses (Bayer-matrix threshold dithering), not an external
// illustration.

const W: usize = 800;
const H: usize = 600;

const BAYER_4X4: [[u32; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

fn set(buf: &mut [f32], x: i32, y: i32, c: [f32; 3]) {
    if x < 0 || y < 0 || x as usize >= W || y as usize >= H {
        return;
    }
    let i = (y as usize * W + x as usize) * 4;
    buf[i] = c[0];
    buf[i + 1] = c[1];
    buf[i + 2] = c[2];
    buf[i + 3] = 1.0;
}

fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(2654435761);
    x ^= x >> 15;
    x = x.wrapping_mul(2246822519);
    x ^= x >> 13;
    (x % 10000) as f32 / 10000.0
}

/// Two-tone Bayer dither between `top`/`bottom` at a chunky `cell`-pixel scale
/// (not per real pixel), so the result reads as deliberate pixel art rather
/// than fine noise.
fn dithered_gradient_pixel(x: i32, y: i32, t: f32, top: [f32; 3], bottom: [f32; 3], cell: i32) -> [f32; 3] {
    let (cx, cy) = ((x / cell) as usize, (y / cell) as usize);
    let threshold = (BAYER_4X4[cy % 4][cx % 4] as f32 + 0.5) / 16.0;
    if t > threshold {
        bottom
    } else {
        top
    }
}

/// 5x7 pixel font, one `u8` per row (5 low bits, MSB = leftmost column).
/// Only the letters "DITHERWAVE" needs are defined.
fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11100, 0b10000, 0b10000, 0b11111],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10010, 0b10001, 0b10001],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        _ => [0; 7],
    }
}

fn draw_text(buf: &mut [f32], text: &str, start_x: i32, start_y: i32, scale: i32, color: [f32; 3]) -> i32 {
    let mut x = start_x;
    for c in text.chars() {
        let g = glyph(c);
        for (row, bits) in g.iter().enumerate() {
            for col in 0..5 {
                if (bits >> (4 - col)) & 1 == 1 {
                    let px = x + col as i32 * scale;
                    let py = start_y + row as i32 * scale;
                    for dy in 0..scale {
                        for dx in 0..scale {
                            set(buf, px + dx, py + dy, color);
                        }
                    }
                }
            }
        }
        x += (5 + 1) * scale; // 5-wide glyph + 1-cell gap
    }
    x - scale // end x (minus the trailing gap)
}

fn text_width(text: &str, scale: i32) -> i32 {
    text.len() as i32 * (5 + 1) * scale - scale
}

/// The waveform-bar logo mark, scaled to `total_h` tall, centered at `cx`.
fn draw_logo(buf: &mut [f32], cx: i32, center_y: i32, total_h: i32, cell: i32) {
    let ratios: [i32; 9] = [4, 8, 13, 19, 24, 19, 13, 8, 4];
    let max_ratio = 24;
    let bar_w = total_h / 6; // proportion matching the app icon (bar width : max height)
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

fn main() {
    let mut buf = vec![0.0f32; W * H * 4];

    // Quiet backdrop: a Bayer-dithered vertical gradient, deep violet to
    // near-black — atmosphere, not a scene, so it doesn't compete with the logo.
    let bg_top = [0.16, 0.08, 0.30];
    let bg_bottom = [0.03, 0.02, 0.06];
    for y in 0..H as i32 {
        let t = y as f32 / H as f32;
        for x in 0..W as i32 {
            let color = dithered_gradient_pixel(x, y, t, bg_top, bg_bottom, 3);
            set(&mut buf, x, y, color);
        }
    }

    // A light scatter of stars for a little life, without becoming a scene.
    for i in 0..60u32 {
        let sx = (hash(i * 7 + 1) * W as f32) as i32;
        let sy = (hash(i * 7 + 2) * (H as f32 * 0.42)) as i32;
        let b = 0.55 + hash(i * 7 + 3) * 0.35;
        set(&mut buf, sx, sy, [b, b, b * 0.95]);
    }

    // Logo mark, large and centered in the upper-middle.
    draw_logo(&mut buf, W as i32 / 2, 230, 220, 5);

    // Wordmark below it, big and legible — this is the focal point.
    let scale = 11;
    let word = "DITHERWAVE";
    let w = text_width(word, scale);
    let start_x = (W as i32 - w) / 2;
    let start_y = 400;
    // Drop shadow for contrast against the dithered backdrop, then the fill.
    draw_text(&mut buf, word, start_x + 4, start_y + 4, scale, [0.03, 0.02, 0.06]);
    draw_text(&mut buf, word, start_x, start_y, scale, [0.97, 0.95, 0.90]);

    let mut out = image::RgbaImage::new(W as u32, H as u32);
    for y in 0..H {
        for x in 0..W {
            let i = (y * W + x) * 4;
            let px = [
                (buf[i] * 255.0).round() as u8,
                (buf[i + 1] * 255.0).round() as u8,
                (buf[i + 2] * 255.0).round() as u8,
                255,
            ];
            out.put_pixel(x as u32, y as u32, image::Rgba(px));
        }
    }

    let out_path = std::env::args().nth(1).unwrap_or_else(|| "demo.png".to_string());
    out.save(&out_path).expect("failed to save demo image");
    println!("wrote {out_path} ({W}x{H})");
}
