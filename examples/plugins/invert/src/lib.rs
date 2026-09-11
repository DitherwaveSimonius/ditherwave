//! Reference plugin: inverts each pixel's RGB channels (alpha untouched),
//! blended against the original by a `strength` parameter. Minimal on
//! purpose — this exists to prove the plugin system end-to-end, not to be a
//! useful effect. See ditheros-plugin-sdk's crate docs for the annotated
//! version of this same code.

use ditheros_plugin_sdk::*;

#[plugin_fn]
pub fn apply(Json(request): Json<PluginRequest>) -> FnResult<Json<PluginResponse>> {
    let pixels = match decode_pixels(&request) {
        Ok(p) => p,
        Err(e) => return Ok(Json(PluginResponse::err(e))),
    };

    let strength = request
        .params
        .get("strength")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0) as f32;

    let mut out = pixels;
    for px in out.chunks_mut(4) {
        for c in px[..3].iter_mut() {
            let original = *c as f32;
            let inverted = 255.0 - original;
            *c = (original + (inverted - original) * strength)
                .round()
                .clamp(0.0, 255.0) as u8;
        }
    }

    Ok(Json(encode_response(&out)))
}
