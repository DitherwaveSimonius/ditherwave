//! End-to-end test of the exact pipeline the Tauri desktop commands drive:
//! decode a real PNG, run it through every built-in effect, re-encode, and
//! write it back to disk. This is the closest thing to a UI smoke test we can
//! run headlessly (no display needed), since `apps/desktop/src-tauri`'s
//! `open_image`/`apply_effect`/`export_image` commands are thin wrappers
//! around exactly these `ditheros_core` calls.

use ditheros_core::{io, EffectContext, Registry};
use std::path::Path;

#[test]
fn every_builtin_effect_runs_end_to_end_on_a_real_image() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample.png");
    let input = io::open_raster(&fixture).expect("fixture should decode");
    assert!(input.width > 0 && input.height > 0);

    let registry = Registry::with_builtins();
    let no_cancel = || false;
    let ctx = EffectContext {
        rng_seed: 0,
        preview: false,
        cancelled: &no_cancel,
    };

    let out_dir = std::env::temp_dir().join("ditheros-core-pipeline-test");
    std::fs::create_dir_all(&out_dir).unwrap();

    for key in registry.keys() {
        let effect = registry.create(key).unwrap();
        let params = Default::default();

        let output = effect
            .apply(&input, &params, &ctx)
            .unwrap_or_else(|e| panic!("effect '{key}' failed: {e}"));

        assert_eq!(
            (output.width, output.height),
            (input.width, input.height),
            "effect '{key}' changed dimensions"
        );
        assert_eq!(
            output.pixels.len(),
            input.pixels.len(),
            "effect '{key}' changed buffer length"
        );
        assert!(
            output.pixels.iter().all(|c| (0.0..=1.0).contains(c)),
            "effect '{key}' produced out-of-range channel values"
        );
        // A dithering effect should actually change a photo-like fixture, not
        // silently pass it through.
        assert!(
            output.pixels != input.pixels,
            "effect '{key}' did not change the image at all"
        );

        let out_path = out_dir.join(format!("{}.png", key.replace('.', "_")));
        io::save_raster(&output, &out_path)
            .unwrap_or_else(|e| panic!("effect '{key}' failed to save: {e}"));
        assert!(
            out_path.metadata().unwrap().len() > 0,
            "effect '{key}' wrote an empty file"
        );

        // Round-trip: re-decoding what we just wrote should match what we encoded.
        let reloaded = io::open_raster(&out_path).unwrap();
        assert_eq!(
            (reloaded.width, reloaded.height),
            (output.width, output.height)
        );
    }
}
