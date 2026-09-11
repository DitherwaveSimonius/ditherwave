//! End-to-end test against the real, compiled example plugin
//! (`examples/plugins/invert`) — the strongest verification available for
//! the plugin system, since it exercises actual WASM instantiation and
//! execution through Extism/Wasmtime, not a mock. Requires
//! `examples/plugins/invert/invert_plugin.wasm` to exist (built via `cargo
//! build -p invert_plugin --target wasm32-wasip1 --release`, then copied
//! next to `plugin.toml` — see `examples/plugins/README.md`).

use ditherwave_core::{ColorSpace, Effect, EffectContext, WorkingImage};
use ditherwave_plugin_host::load_plugin;
use std::path::Path;

fn invert_manifest_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/plugins/invert/plugin.toml")
}

#[test]
fn loads_and_runs_the_example_invert_plugin() {
    let manifest = invert_manifest_path();
    let plugin = load_plugin(&manifest).unwrap_or_else(|e| {
        panic!(
            "failed to load example plugin at '{}': {e}",
            manifest.display()
        )
    });

    assert_eq!(plugin.key(), "examples.invert");
    assert_eq!(plugin.display_name(), "Invert (example plugin)");
    assert_eq!(plugin.params_schema().len(), 1);
    assert_eq!(plugin.params_schema()[0].key, "strength");

    // 1x1 image: mid-gray-ish, alpha half. Full-strength invert should flip
    // RGB but leave alpha untouched.
    let input = WorkingImage {
        width: 1,
        height: 1,
        color_space: ColorSpace::Srgb,
        pixels: vec![0.2, 0.4, 0.6, 0.5],
    };
    let no_cancel = || false;
    let ctx = EffectContext {
        rng_seed: 0,
        preview: false,
        cancelled: &no_cancel,
    };

    let mut params = ditherwave_core::ParamValues::new();
    params.insert("strength".to_string(), serde_json::json!(1.0));

    let output = plugin
        .apply(&input, &params, &ctx)
        .expect("plugin call should succeed");

    assert_eq!((output.width, output.height), (1, 1));
    // Compare in u8 space since the plugin round-trips through RGBA8.
    let to_u8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as i32;
    assert_eq!(to_u8(output.pixels[0]), 255 - to_u8(input.pixels[0]));
    assert_eq!(to_u8(output.pixels[1]), 255 - to_u8(input.pixels[1]));
    assert_eq!(to_u8(output.pixels[2]), 255 - to_u8(input.pixels[2]));
    assert_eq!(
        to_u8(output.pixels[3]),
        to_u8(input.pixels[3]),
        "alpha must be untouched"
    );
}

#[test]
fn strength_zero_leaves_the_image_unchanged() {
    let manifest = invert_manifest_path();
    let plugin = load_plugin(&manifest).expect("failed to load example plugin");

    let input = WorkingImage {
        width: 1,
        height: 1,
        color_space: ColorSpace::Srgb,
        pixels: vec![0.3, 0.7, 0.1, 1.0],
    };
    let no_cancel = || false;
    let ctx = EffectContext {
        rng_seed: 0,
        preview: false,
        cancelled: &no_cancel,
    };

    let mut params = ditherwave_core::ParamValues::new();
    params.insert("strength".to_string(), serde_json::json!(0.0));

    let output = plugin
        .apply(&input, &params, &ctx)
        .expect("plugin call should succeed");
    let to_u8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as i32;
    for c in 0..4 {
        assert_eq!(to_u8(output.pixels[c]), to_u8(input.pixels[c]));
    }
}

#[test]
fn plugin_runs_through_the_registry_and_effect_stack_like_a_builtin() {
    // Proves a loaded plugin is indistinguishable from a built-in effect
    // from the registry/stack's point of view — the whole reason
    // Registry::register takes an already-built instance rather than a
    // constructor fn (a loaded WASM module is stateful and expensive to
    // reconstruct on every lookup).
    let manifest = invert_manifest_path();
    let adapter = load_plugin(&manifest).expect("failed to load example plugin");
    let key = adapter.key();

    let mut registry = ditherwave_core::Registry::with_builtins();
    registry.register(key, Box::new(adapter));
    assert!(registry.get(key).is_some());

    let stack = ditherwave_core::EffectStack {
        nodes: vec![ditherwave_core::EffectNode {
            id: "1".into(),
            effect_key: key.to_string(),
            params: [("strength".to_string(), serde_json::json!(1.0))]
                .into_iter()
                .collect(),
            enabled: true,
        }],
    };
    let input = WorkingImage {
        width: 1,
        height: 1,
        color_space: ColorSpace::Srgb,
        pixels: vec![0.0, 0.0, 0.0, 1.0],
    };

    let output = stack
        .run(&input, &registry, 0)
        .expect("stack should run the plugin");
    let to_u8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as i32;
    assert_eq!(to_u8(output.pixels[0]), 255, "black should invert to white");
}
