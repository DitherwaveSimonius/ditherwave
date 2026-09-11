# Branding

`logo.png` (1024x1024) is the source icon for the whole project: app icons,
window icon, and website/favicon assets are all generated from it.

## Design

A Bayer-dithered violet-to-black vertical gradient (the same 4x4 ordered-dithering
matrix `ditherwave-core`'s `ordered::Bayer4x4` effect uses) behind a cream
equalizer/waveform silhouette — built at a 32x32 "pixel grid" and upscaled with
nearest-neighbor, for the chunky, visibly-pixelated look the whole app's design
takes inspiration from.

## Regenerating

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
