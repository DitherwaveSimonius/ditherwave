# Ditherwave

Open-source dithering app for photos (raster + RAW) and vector graphics: error-diffusion,
ordered/Bayer, pattern and glitch effects in a stackable, non-destructive pipeline, with
a CLI for batch processing and a WASM plugin system for custom algorithms.

Inspired by the (commercial, closed-source) [Dither Boy](https://studioaaa.com/product/dither-boy/)
by Studio AAA — this project shares no code with it and aims to be a more capable, fully
open-source alternative, not a clone.

## Status

Milestones M0-M7 done: raster (PNG/JPEG), RAW, and SVG input, all through the
same non-destructive, reorderable effect stack. 19 built-in effects across 5
categories (7 error-diffusion, 3 ordered/Bayer, 3 pattern/clustered-dot
halftone, 1 palette-map with 2 built-in palettes + k-means extraction, 2
glitch, 1 special/glow). A stack built in the desktop app can be saved as a
TOML recipe and replayed over a whole folder with `ditherwave-cli`, or
exported as a "dot emission" vector SVG for print/embroidery. Custom
algorithms can be loaded as sandboxed WASM plugins (see `examples/plugins/`)
— they show up in the effect list and work in stacks/recipes exactly like a
built-in. CI also builds/bundles the desktop app on Windows. Not done yet:
animation/video dithering, contour vectorization — see the roadmap below.

## Project layout

```
crates/
  core/         effect trait, working-image type, effect stack, built-in algorithms
  raw/          RAW photo decoding -> working image
  vector/       SVG input + vector export of dithered output
  plugin-api/   shared host<->WASM plugin types
  plugin-host/  WASM plugin loader
  recipe/       serializable effect-stack "recipes" shared by the GUI and CLI
apps/
  desktop/      Tauri (Rust + Svelte) desktop app
  cli/          batch/folder processing binary
sdk/plugin-sdk-rust/   template for Rust-based plugin authors
examples/plugins/      reference WASM plugins (see examples/plugins/README.md)
```

## Development

Requires Rust (stable, via [rustup](https://rustup.rs)) and Node.js (via
[nvm](https://github.com/nvm-sh/nvm) or your package manager of choice).

```bash
# whole Rust workspace
cargo check --workspace
cargo clippy --workspace --all-targets
cargo fmt --all

# desktop app
cd apps/desktop
npm install
npm run tauri dev

# batch-apply a recipe (exported from the desktop app's "Save Recipe…") to a folder
cargo run -p ditherwave-cli -- --recipe my-look.toml --input ./photos --output ./out
```

Both the desktop app and the CLI load WASM plugins from
`~/Library/Application Support/ditherwave/plugins` (macOS) at startup — see
`examples/plugins/README.md` for the plugin format and how to build one.

## Branding

The app icon and in-app wordmark are generated, not hand-drawn — see
`branding/README.md` for the design and how to regenerate them.

## Roadmap

- **M0** — workspace scaffold, CI, empty desktop window
- **M1** — minimal raster pipeline (a handful of algorithms, open/export, basic UI)
- **M2** — full non-destructive effect stack, remaining built-in algorithms, palettes
- **M3** — RAW input
- **M4** — recipes + CLI/batch processing
- **M5** — SVG input + vector (dot-emission) export
- **M6** — WASM plugin system
- **M7** — Windows build + packaging
- **M8** — animation/video dithering, contour-vectorization (stretch)

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

`crates/raw` depends on [`imagepipe`](https://crates.io/crates/imagepipe)
(LGPL-3.0-only) for RAW decoding, used unmodified as an ordinary Cargo
dependency — this doesn't require the rest of the project to be
LGPL/GPL-licensed, but is worth a proper compliance check (attribution,
ability to relink/replace that component) before a public binary release.
