//! Helper crate for writing ditheros plugins in Rust. A plugin is a small
//! WASM module (built for `wasm32-wasip1`) exporting one function, `apply`,
//! wired up with `#[plugin_fn]` from `extism-pdk` (re-exported here so
//! plugin authors don't need it as a separate dependency).
//!
//! ```ignore
//! use ditheros_plugin_sdk::*;
//!
//! #[plugin_fn]
//! pub fn apply(Json(request): Json<PluginRequest>) -> FnResult<Json<PluginResponse>> {
//!     let mut pixels = decode_pixels(&request)?;
//!     for px in pixels.chunks_mut(4) {
//!         px[0] = 255 - px[0];
//!         px[1] = 255 - px[1];
//!         px[2] = 255 - px[2];
//!         // px[3] (alpha) left untouched
//!     }
//!     Ok(Json(encode_response(&pixels)))
//! }
//! ```
//!
//! See `examples/plugins/invert` for the complete, buildable version of the
//! above, including its `plugin.toml` manifest.

pub use ditheros_plugin_api::{PluginRequest, PluginResponse};
pub use extism_pdk::{self, plugin_fn, FnResult, Json};

use base64::Engine;

/// Decodes a [`PluginRequest`]'s pixel buffer to raw RGBA8 bytes
/// (`width * height * 4` bytes, row-major).
pub fn decode_pixels(request: &PluginRequest) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD
        .decode(&request.pixels_base64)
        .map_err(|e| format!("invalid pixels_base64: {e}"))
}

/// Wraps a transformed RGBA8 pixel buffer into a success [`PluginResponse`].
pub fn encode_response(pixels: &[u8]) -> PluginResponse {
    PluginResponse::ok(base64::engine::general_purpose::STANDARD.encode(pixels))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_then_encode_round_trips_bytes() {
        let original = vec![10u8, 20, 30, 255, 40, 50, 60, 255];
        let request = PluginRequest {
            width: 2,
            height: 1,
            pixels_base64: base64::engine::general_purpose::STANDARD.encode(&original),
            params: Default::default(),
            seed: 0,
        };

        let decoded = decode_pixels(&request).unwrap();
        assert_eq!(decoded, original);

        let response = encode_response(&decoded);
        assert_eq!(
            response.pixels_base64.as_deref(),
            Some(request.pixels_base64.as_str())
        );
    }

    #[test]
    fn decode_pixels_rejects_invalid_base64() {
        let request = PluginRequest {
            width: 1,
            height: 1,
            pixels_base64: "not valid base64!!".into(),
            params: Default::default(),
            seed: 0,
        };
        assert!(decode_pixels(&request).is_err());
    }
}
