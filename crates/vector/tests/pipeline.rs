//! End-to-end test of the SVG pipeline the desktop app actually drives: open
//! an SVG, run it through a real effect stack, and export the result as
//! dot-emission vector SVG — mirroring `ditherwave-core`'s own pipeline test.

use ditherwave_core::{EffectNode, EffectStack, Registry};
use ditherwave_vector::{export_dots, open_svg};
use std::path::Path;

#[test]
fn svg_in_stack_dots_out_end_to_end() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shapes.svg");
    let input = open_svg(&fixture, None).expect("fixture should rasterize");
    assert_eq!((input.width, input.height), (64, 64));

    let registry = Registry::with_builtins();
    let stack = EffectStack {
        nodes: vec![EffectNode {
            id: "1".into(),
            effect_key: "color.palette_map".into(),
            params: [
                ("palette".to_string(), serde_json::json!("pico8")),
                ("dither".to_string(), serde_json::json!(true)),
            ]
            .into_iter()
            .collect(),
            enabled: true,
        }],
    };

    let rendered = stack
        .run(&input, &registry, 0)
        .expect("stack should run on rasterized SVG");
    assert_eq!((rendered.width, rendered.height), (64, 64));

    let out_dir = std::env::temp_dir().join("ditherwave-vector-pipeline-test");
    std::fs::create_dir_all(&out_dir).unwrap();
    let out_path = out_dir.join("shapes_dots.svg");

    export_dots(&rendered, &out_path, 0.45, [0.0, 0.0, 0.0]).expect("dot export should succeed");

    let svg = std::fs::read_to_string(&out_path).unwrap();
    assert!(svg.contains("<svg"));
    assert!(
        svg.matches("<circle").count() > 100,
        "expected many dots across a 64x64 dithered image"
    );

    // The exported dots-SVG must itself be valid, re-openable SVG.
    let reopened = open_svg(&out_path, None).expect("exported dots SVG should itself be valid SVG");
    assert_eq!((reopened.width, reopened.height), (64, 64));
}
