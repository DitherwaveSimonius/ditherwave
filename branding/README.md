# Branding

Two generated assets, both produced by small standalone Rust tools rather than
hand-edited images — regenerating them after a design tweak is a `cargo run` away.

## `logo.png` (1024x1024)

The app icon / window icon / favicon source: individual waveform bars, each
filled with a Bayer-dithered violet-to-cream gradient, on a transparent
background — freestanding, no card/background box behind it.

```bash
cd branding/generate-logo
cargo run --release -- ../logo.png
```

Then regenerate the desktop app's full icon set from the new `logo.png`:

```bash
cd apps/desktop
npx tauri icon ../../branding/logo.png
# icons/ios and icons/android are generated too — delete them, we don't
# target mobile:
rm -rf src-tauri/icons/ios src-tauri/icons/android
```

And update the web favicon:

```bash
sips -Z 64 branding/logo.png --out apps/desktop/public/favicon.png
```

## `demo.png` (800x600)

The image the desktop app loads by default on first launch (before the user
opens their own photo) — the logo mark and the "DITHERWAVE" wordmark (a
hand-drawn 5x7 pixel font), large and centered, in front of a deliberately
quiet Bayer-dithered violet-to-black backdrop. The brand is the subject; the
backdrop is just atmosphere.

```bash
cd branding/generate-demo
cargo run --release -- ../demo.png
cp ../demo.png ../../apps/desktop/src-tauri/assets/demo.png
```

The app embeds this file at compile time (`include_bytes!`), so a rebuild is
needed after regenerating it.
