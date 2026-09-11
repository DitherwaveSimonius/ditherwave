# Example Plugins

Reference WASM plugins for `ditherwave`, built with [`ditherwave-plugin-sdk`](../../sdk/plugin-sdk-rust).
A plugin is a directory with two files:

- `plugin.toml` — the manifest (key, display name, category, parameter schema, and
  which `.wasm` file to load, relative to the manifest)
- the compiled `.wasm` module itself

## invert

The minimal complete example: inverts RGB (alpha untouched), blended against the
original by a `strength` parameter. It exists to prove the plugin system works
end to end, not to be a useful effect.

`invert_plugin.wasm` is committed pre-built, so the example loads out of the box.
To rebuild it after changing `src/lib.rs`:

```bash
rustup target add wasm32-wasip1   # once
cargo build -p invert_plugin --target wasm32-wasip1 --release
cp target/wasm32-wasip1/release/invert_plugin.wasm examples/plugins/invert/
```

## Writing your own

1. `cargo new --lib my_plugin` inside (or outside) this repo, add
   `ditherwave-plugin-sdk` as a dependency and `crate-type = ["cdylib"]`.
2. Export one function using `#[plugin_fn]` — see `invert/src/lib.rs` for the
   complete pattern (decode the request's pixels, transform them, re-encode a
   response).
3. Write a `plugin.toml` next to it: a `[plugin]` table (`key`, `display_name`,
   `category` — one of `error_diffusion`, `ordered`, `pattern`, `glitch`,
   `special`, `color` — `version`, `author`, `wasm`), plus one `[[params]]`
   entry per parameter (`key`, `display_name`, `kind` — `bool`, `int`, `float`,
   or `choice` — and that kind's `min`/`max`/`step`/`options`/`default`).
4. Build for `wasm32-wasip1`, copy the `.wasm` next to `plugin.toml`, and drop
   the whole directory into ditherwave's plugins folder
   (`~/Library/Application Support/ditherwave/plugins` on macOS).

Plugins run sandboxed: by default they get no filesystem or network access —
only the pixel buffer and parameters passed to `apply`, and only your
transformed pixel buffer comes back out.
