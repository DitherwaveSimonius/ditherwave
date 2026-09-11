//! SVG input (rasterized via `resvg`/`usvg` into the normal pixel pipeline)
//! and "dot emission" vector export of an already-dithered image — see the
//! project plan's note on why general vector-native dithering isn't
//! attempted: most dithering algorithms (error diffusion, ordered) are
//! fundamentally per-pixel operations with no meaningful vector equivalent.
//! What *is* useful and achievable is exporting the dithered raster result
//! as discrete vector dots, which is what print and embroidery workflows
//! actually want.

use ditherwave_core::{ColorSpace, WorkingImage};
use resvg::{tiny_skia, usvg};
use std::fmt::Write as _;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum VectorError {
    #[error("failed to parse SVG: {0}")]
    Parse(#[from] usvg::Error),
    #[error("failed to allocate a render target for a {0}x{1} image")]
    Pixmap(u32, u32),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Rasterizes an SVG file into a [`WorkingImage`]. If `target` is given, the
/// SVG is scaled to exactly those pixel dimensions; otherwise it's rendered
/// at its own intrinsic size. Text rendering uses `usvg`'s default (empty)
/// font database, so `<text>` elements may not render as intended — fine for
/// the icon/illustration SVGs this is meant for.
pub fn open_svg(path: &Path, target: Option<(u32, u32)>) -> Result<WorkingImage, VectorError> {
    let svg_text = std::fs::read_to_string(path)?;
    let tree = usvg::Tree::from_str(&svg_text, &usvg::Options::default())?;

    let size = tree.size();
    let (out_w, out_h) = target.unwrap_or_else(|| {
        (
            size.width().round().max(1.0) as u32,
            size.height().round().max(1.0) as u32,
        )
    });

    let mut pixmap =
        tiny_skia::Pixmap::new(out_w, out_h).ok_or(VectorError::Pixmap(out_w, out_h))?;
    let transform =
        tiny_skia::Transform::from_scale(out_w as f32 / size.width(), out_h as f32 / size.height());
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Pixmap stores premultiplied alpha; WorkingImage/PNG expect straight alpha.
    let bytes = pixmap.take_demultiplied();
    let pixels = bytes.into_iter().map(|b| b as f32 / 255.0).collect();
    Ok(WorkingImage {
        width: out_w,
        height: out_h,
        color_space: ColorSpace::Srgb,
        pixels,
    })
}

fn approx_eq(a: [f32; 3], b: [f32; 3]) -> bool {
    const EPSILON: f32 = 0.02;
    (0..3).all(|i| (a[i] - b[i]).abs() < EPSILON)
}

/// Exports `image` as an SVG of small dots, one per pixel that doesn't match
/// `background`, grouped into one `<g>` per distinct color — every pixel
/// becomes a plottable dot, which is what print/embroidery workflows expect
/// from dithered artwork. Meant for an already-dithered, low-color image;
/// running this on a photo with smooth gradients will produce one dot per
/// pixel with no two alike, which is technically correct but not useful.
pub fn export_dots(
    image: &WorkingImage,
    path: &Path,
    dot_radius: f32,
    background: [f32; 3],
) -> Result<(), VectorError> {
    use std::collections::BTreeMap;

    let mut groups: BTreeMap<[u8; 3], Vec<(u32, u32)>> = BTreeMap::new();
    for y in 0..image.height {
        for x in 0..image.width {
            let base = ((y * image.width + x) * 4) as usize;
            let color = [
                image.pixels[base],
                image.pixels[base + 1],
                image.pixels[base + 2],
            ];
            if approx_eq(color, background) {
                continue;
            }
            let key = [
                (color[0].clamp(0.0, 1.0) * 255.0).round() as u8,
                (color[1].clamp(0.0, 1.0) * 255.0).round() as u8,
                (color[2].clamp(0.0, 1.0) * 255.0).round() as u8,
            ];
            groups.entry(key).or_default().push((x, y));
        }
    }

    let mut svg = String::new();
    let _ = writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#,
        w = image.width,
        h = image.height
    );
    for (color, points) in &groups {
        let _ = writeln!(
            svg,
            r#"  <g fill="rgb({},{},{})">"#,
            color[0], color[1], color[2]
        );
        for (x, y) in points {
            let _ = writeln!(
                svg,
                r#"    <circle cx="{:.2}" cy="{:.2}" r="{:.2}"/>"#,
                *x as f32 + 0.5,
                *y as f32 + 0.5,
                dot_radius
            );
        }
        let _ = writeln!(svg, "  </g>");
    }
    svg.push_str("</svg>\n");

    Ok(std::fs::write(path, svg)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ditherwave_core::ColorSpace;

    const RED_SQUARE_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">
        <rect width="10" height="10" fill="rgb(255,0,0)"/>
    </svg>"#;

    fn write_temp_svg(name: &str, contents: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn open_svg_rasterizes_at_intrinsic_size() {
        let path = write_temp_svg("ditherwave-vector-test-intrinsic.svg", RED_SQUARE_SVG);
        let image = open_svg(&path, None).unwrap();
        assert_eq!((image.width, image.height), (10, 10));
        // Center pixel should be solid red.
        let base = ((5 * image.width + 5) * 4) as usize;
        assert!(image.pixels[base] > 0.9);
        assert!(image.pixels[base + 1] < 0.1);
        assert!(image.pixels[base + 2] < 0.1);
    }

    #[test]
    fn open_svg_rasterizes_at_requested_target_size() {
        let path = write_temp_svg("ditherwave-vector-test-scaled.svg", RED_SQUARE_SVG);
        let image = open_svg(&path, Some((40, 20))).unwrap();
        assert_eq!((image.width, image.height), (40, 20));
    }

    #[test]
    fn open_svg_rejects_invalid_svg() {
        let path = write_temp_svg("ditherwave-vector-test-invalid.svg", "not an svg at all");
        assert!(open_svg(&path, None).is_err());
    }

    #[test]
    fn export_dots_skips_background_and_groups_by_color() {
        // 2x1: one white (background) pixel, one red pixel.
        let image = WorkingImage {
            width: 2,
            height: 1,
            color_space: ColorSpace::Srgb,
            pixels: vec![1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 1.0],
        };
        let out_path = std::env::temp_dir().join("ditherwave-vector-test-dots.svg");

        export_dots(&image, &out_path, 0.5, [1.0, 1.0, 1.0]).unwrap();
        let svg = std::fs::read_to_string(&out_path).unwrap();

        assert_eq!(
            svg.matches("<circle").count(),
            1,
            "only the non-background pixel should emit a dot"
        );
        assert!(svg.contains("rgb(255,0,0)"));
        assert!(
            !svg.contains("rgb(255,255,255)"),
            "background color must not get its own group"
        );
    }

    #[test]
    fn export_dots_on_an_all_background_image_produces_no_circles() {
        let image = WorkingImage::new(3, 3, ColorSpace::Srgb);
        let out_path = std::env::temp_dir().join("ditherwave-vector-test-empty-dots.svg");

        export_dots(&image, &out_path, 0.5, [0.0, 0.0, 0.0]).unwrap();
        let svg = std::fs::read_to_string(&out_path).unwrap();

        assert_eq!(svg.matches("<circle").count(), 0);
        assert!(svg.contains("<svg"));
    }
}
