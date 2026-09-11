# ditheros

Open-source dithering app for photos (raster + RAW) and vector graphics: error-diffusion,
ordered/Bayer, pattern and glitch effects in a stackable, non-destructive pipeline, with
a CLI for batch processing and a WASM plugin system for custom algorithms.

> `ditheros` is a working project name, not final — see below.

Inspired by the (commercial, closed-source) [Dither Boy](https://studioaaa.com/product/dither-boy/)
by Studio AAA. This project shares no code with it and aims to be a more capable, fully
open-source alternative — not a clone. **The final project name must not be "Dither
Boy"/"DitherBoy" or a close variant**, to avoid colliding with that product's name.

## Status

Milestones M0-M5 done: raster (PNG/JPEG), RAW, and now SVG input, all through
the same non-destructive, reorderable effect stack. 14 built-in algorithms (7
error-diffusion, 3 ordered/Bayer, 3 pattern/clustered-dot halftone, 1
palette-map with 2 built-in palettes + k-means extraction). A stack built in
the desktop app can be saved as a TOML recipe and replayed over a whole
folder with `ditheros-cli`, or exported as a "dot emission" vector SVG for
print/embroidery. No plugin system yet — see the roadmap below.

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
examples/plugins/      reference plugins (added starting M6)
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
cargo run -p ditheros-cli -- --recipe my-look.toml --input ./photos --output ./out
```

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
