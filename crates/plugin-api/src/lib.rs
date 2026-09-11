//! The wire contract between the host (`ditheros-plugin-host`, running in the
//! desktop app / CLI) and a plugin (any WASM module, built with
//! `ditheros-plugin-sdk` or any other language's Extism PDK). Both sides
//! agree only on these two JSON-serialized types — nothing here depends on
//! `ditheros-core`, so a plugin author's crate doesn't need to pull in image
//! decoding, the effect registry, or anything else host-side.
//!
//! A plugin exports one function, `apply`, taking a [`PluginRequest`] and
//! returning a [`PluginResponse`] (both JSON-encoded over the Extism call
//! boundary — see `ditheros-plugin-sdk` for the guest-side helper and
//! `ditheros-plugin-host` for the host-side caller).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One call to a plugin's `apply` export: an RGBA8 image plus whatever
/// parameters the plugin's manifest declared.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRequest {
    pub width: u32,
    pub height: u32,
    /// Row-major RGBA, one byte per channel, base64-encoded. Raw bytes would
    /// bloat the JSON payload less, but base64 keeps the wire format uniform
    /// and human-inspectable for a v1 plugin system; revisit if this becomes
    /// a real bottleneck.
    pub pixels_base64: String,
    /// Plugin-declared parameter values, keyed by the manifest's param keys.
    pub params: HashMap<String, serde_json::Value>,
    /// Deterministic seed for any randomized effect (e.g. glitch/pattern
    /// plugins), so the same input+params+seed always produces the same output.
    pub seed: u64,
}

/// A plugin's `apply` result: either a same-size, same-format pixel buffer,
/// or an error message. v1 plugins cannot resize or reformat the image —
/// only transform pixels in place.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pixels_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl PluginResponse {
    pub fn ok(pixels_base64: String) -> Self {
        Self {
            pixels_base64: Some(pixels_base64),
            error: None,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            pixels_base64: None,
            error: Some(message.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_round_trips_through_json() {
        let request = PluginRequest {
            width: 2,
            height: 1,
            pixels_base64: "AAAA".into(),
            params: HashMap::from([("intensity".to_string(), serde_json::json!(0.5))]),
            seed: 42,
        };
        let bytes = serde_json::to_vec(&request).unwrap();
        let parsed: PluginRequest = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed.width, 2);
        assert_eq!(parsed.seed, 42);
        assert_eq!(
            parsed.params.get("intensity"),
            Some(&serde_json::json!(0.5))
        );
    }

    #[test]
    fn ok_and_err_responses_round_trip_through_json() {
        let ok = PluginResponse::ok("QUJD".into());
        let ok_bytes = serde_json::to_vec(&ok).unwrap();
        let ok_parsed: PluginResponse = serde_json::from_slice(&ok_bytes).unwrap();
        assert_eq!(ok_parsed.pixels_base64.as_deref(), Some("QUJD"));
        assert!(ok_parsed.error.is_none());

        let err = PluginResponse::err("boom");
        let err_bytes = serde_json::to_vec(&err).unwrap();
        let err_parsed: PluginResponse = serde_json::from_slice(&err_bytes).unwrap();
        assert!(err_parsed.pixels_base64.is_none());
        assert_eq!(err_parsed.error.as_deref(), Some("boom"));
    }
}
