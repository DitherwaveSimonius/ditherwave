//! Loads user-provided WASM plugins (via [Extism](https://extism.org)) and
//! wraps each one in a [`ditherwave_core::Effect`], so plugins show up in the
//! registry exactly like a built-in algorithm — the rest of the app (UI,
//! stack, recipes, CLI) never needs to know an effect came from a plugin.
//!
//! A plugin is a directory containing a `plugin.toml` manifest and the
//! `.wasm` file it points to. Plugins run sandboxed: by default they get no
//! filesystem or network access (Extism/Wasmtime's linear-memory isolation
//! plus no host functions registered), only the pixel buffer and parameters
//! passed to `apply`. See `examples/plugins/invert` for a complete example.

use base64::Engine;
use ditherwave_core::{
    Effect, EffectCategory, EffectContext, EffectError, EffectResult, ParamDef, ParamKind,
    ParamValues, WorkingImage,
};
use ditherwave_plugin_api::{PluginRequest, PluginResponse};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("failed to parse manifest: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("invalid manifest: {0}")]
    Manifest(String),
    #[error("failed to load WASM module: {0}")]
    Extism(String),
}

#[derive(Debug, Deserialize)]
struct ManifestFile {
    plugin: ManifestPlugin,
    #[serde(default)]
    params: Vec<ManifestParam>,
}

#[derive(Debug, Deserialize)]
struct ManifestPlugin {
    key: String,
    display_name: String,
    category: String,
    #[serde(default)]
    #[allow(dead_code)]
    // surfaced to users via a future "plugin info" UI, not needed to run one
    version: String,
    #[serde(default)]
    #[allow(dead_code)]
    author: String,
    wasm: String,
}

#[derive(Debug, Deserialize)]
struct ManifestParam {
    key: String,
    display_name: String,
    kind: String,
    #[serde(default)]
    min: Option<f64>,
    #[serde(default)]
    max: Option<f64>,
    #[serde(default)]
    step: Option<f64>,
    #[serde(default)]
    options: Option<Vec<String>>,
    default: toml::Value,
}

fn leak_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn parse_category(s: &str) -> Result<EffectCategory, PluginError> {
    Ok(match s {
        "error_diffusion" => EffectCategory::ErrorDiffusion,
        "ordered" => EffectCategory::Ordered,
        "pattern" => EffectCategory::Pattern,
        "glitch" => EffectCategory::Glitch,
        "special" => EffectCategory::Special,
        "color" => EffectCategory::Color,
        other => {
            return Err(PluginError::Manifest(format!(
                "unknown category '{other}' (expected one of: error_diffusion, ordered, pattern, glitch, special, color)"
            )))
        }
    })
}

fn build_param_def(p: &ManifestParam) -> Result<ParamDef, PluginError> {
    let missing = |field: &str| {
        PluginError::Manifest(format!(
            "param '{}': '{}' kind requires '{field}'",
            p.key, p.kind
        ))
    };

    let kind = match p.kind.as_str() {
        "bool" => {
            let default = p
                .default
                .as_bool()
                .ok_or_else(|| missing("a boolean default"))?;
            ParamKind::Bool { default }
        }
        "int" => {
            let min = p.min.ok_or_else(|| missing("min"))? as i64;
            let max = p.max.ok_or_else(|| missing("max"))? as i64;
            let step = p.step.unwrap_or(1.0) as i64;
            let default = p
                .default
                .as_integer()
                .ok_or_else(|| missing("an integer default"))?;
            ParamKind::IntRange {
                min,
                max,
                step,
                default,
            }
        }
        "float" => {
            let min = p.min.ok_or_else(|| missing("min"))?;
            let max = p.max.ok_or_else(|| missing("max"))?;
            let step = p.step.unwrap_or(0.01);
            let default = p
                .default
                .as_float()
                .ok_or_else(|| missing("a float default"))?;
            ParamKind::FloatRange {
                min,
                max,
                step,
                default,
            }
        }
        "choice" => {
            let options = p.options.clone().ok_or_else(|| missing("options"))?;
            let leaked: Vec<&'static str> = options.into_iter().map(leak_str).collect();
            let options: &'static [&'static str] = Box::leak(leaked.into_boxed_slice());
            let default = p
                .default
                .as_str()
                .ok_or_else(|| missing("a string default"))?;
            ParamKind::Choice {
                options,
                default: leak_str(default.to_string()),
            }
        }
        other => {
            return Err(PluginError::Manifest(format!(
                "param '{}': unknown kind '{other}' (expected one of: bool, int, float, choice)",
                p.key
            )))
        }
    };

    Ok(ParamDef {
        key: leak_str(p.key.clone()),
        display_name: leak_str(p.display_name.clone()),
        kind,
    })
}

/// Adapts one loaded WASM plugin to the [`Effect`] trait, so the registry and
/// the rest of the app can use it exactly like a built-in algorithm.
pub struct WasmEffectAdapter {
    key: &'static str,
    display_name: &'static str,
    category: EffectCategory,
    params_schema: &'static [ParamDef],
    plugin: Mutex<extism::Plugin>,
}

impl Effect for WasmEffectAdapter {
    fn key(&self) -> &'static str {
        self.key
    }

    fn display_name(&self) -> &'static str {
        self.display_name
    }

    fn category(&self) -> EffectCategory {
        self.category
    }

    fn params_schema(&self) -> &'static [ParamDef] {
        self.params_schema
    }

    fn apply(
        &self,
        input: &WorkingImage,
        params: &ParamValues,
        ctx: &EffectContext,
    ) -> EffectResult<WorkingImage> {
        let rgba8: Vec<u8> = input
            .pixels
            .iter()
            .map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8)
            .collect();
        let request = PluginRequest {
            width: input.width,
            height: input.height,
            pixels_base64: base64::engine::general_purpose::STANDARD.encode(&rgba8),
            params: params.clone(),
            seed: ctx.rng_seed,
        };
        let input_bytes =
            serde_json::to_vec(&request).map_err(|e| EffectError::Plugin(e.to_string()))?;

        let output_bytes: Vec<u8> = {
            let mut plugin = self.plugin.lock().map_err(|_| {
                EffectError::Plugin("plugin mutex poisoned by a previous panic".into())
            })?;
            plugin
                .call("apply", input_bytes.as_slice())
                .map_err(|e| EffectError::Plugin(e.to_string()))?
        };

        let response: PluginResponse = serde_json::from_slice(&output_bytes)
            .map_err(|e| EffectError::Plugin(e.to_string()))?;
        if let Some(err) = response.error {
            return Err(EffectError::Plugin(err));
        }
        let pixels_base64 = response.pixels_base64.ok_or_else(|| {
            EffectError::Plugin("plugin returned neither pixels nor an error".into())
        })?;
        let rgba8_out = base64::engine::general_purpose::STANDARD
            .decode(&pixels_base64)
            .map_err(|e| EffectError::Plugin(format!("invalid pixels in plugin response: {e}")))?;
        if rgba8_out.len() != input.pixels.len() {
            return Err(EffectError::Plugin(format!(
                "plugin returned {} bytes, expected {} for a {}x{} image",
                rgba8_out.len(),
                input.pixels.len(),
                input.width,
                input.height
            )));
        }

        let pixels: Vec<f32> = rgba8_out.iter().map(|b| *b as f32 / 255.0).collect();
        Ok(WorkingImage {
            width: input.width,
            height: input.height,
            color_space: input.color_space,
            pixels,
        })
    }
}

/// Loads one plugin from its `plugin.toml` manifest path.
pub fn load_plugin(manifest_path: &Path) -> Result<WasmEffectAdapter, PluginError> {
    let manifest_str = std::fs::read_to_string(manifest_path)?;
    let manifest: ManifestFile = toml::from_str(&manifest_str)?;
    let dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let wasm_path = dir.join(&manifest.plugin.wasm);

    let key = leak_str(manifest.plugin.key);
    let display_name = leak_str(manifest.plugin.display_name);
    let category = parse_category(&manifest.plugin.category)?;
    let params = manifest
        .params
        .iter()
        .map(build_param_def)
        .collect::<Result<Vec<_>, _>>()?;
    let params_schema: &'static [ParamDef] = Box::leak(params.into_boxed_slice());

    let extism_manifest = extism::Manifest::new([extism::Wasm::file(&wasm_path)]);
    // `with_wasi: true` — the SDK's WASI target needs it for basic runtime
    // support even though plugins get no filesystem/network host functions.
    let plugin = extism::Plugin::new(&extism_manifest, [], true)
        .map_err(|e| PluginError::Extism(format!("{} ({}): {e}", key, wasm_path.display())))?;

    Ok(WasmEffectAdapter {
        key,
        display_name,
        category,
        params_schema,
        plugin: Mutex::new(plugin),
    })
}

/// Loads every plugin found directly under `dir` (each in its own
/// subdirectory containing a `plugin.toml`). A plugin that fails to load is
/// skipped with a message on stderr rather than failing the whole batch — one
/// broken plugin shouldn't take down every other one.
pub fn load_plugins_dir(dir: &Path) -> Vec<Box<dyn Effect>> {
    let mut plugins: Vec<Box<dyn Effect>> = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return plugins;
    };
    for entry in entries.flatten() {
        let manifest_path = entry.path().join("plugin.toml");
        if !manifest_path.is_file() {
            continue;
        }
        match load_plugin(&manifest_path) {
            Ok(adapter) => plugins.push(Box::new(adapter)),
            Err(e) => eprintln!(
                "ditherwave: failed to load plugin at '{}': {e}",
                manifest_path.display()
            ),
        }
    }
    plugins
}

/// The default per-user plugins directory: platform app-data dir + `ditherwave/plugins`
/// (e.g. `~/Library/Application Support/ditherwave/plugins` on macOS,
/// `%APPDATA%\ditherwave\plugins` on Windows). `None` if the platform's data
/// directory can't be determined.
pub fn default_plugins_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("ditherwave").join("plugins"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_category_accepts_all_documented_values_and_rejects_others() {
        for (s, expected) in [
            ("error_diffusion", EffectCategory::ErrorDiffusion),
            ("ordered", EffectCategory::Ordered),
            ("pattern", EffectCategory::Pattern),
            ("glitch", EffectCategory::Glitch),
            ("special", EffectCategory::Special),
            ("color", EffectCategory::Color),
        ] {
            assert_eq!(parse_category(s).unwrap(), expected);
        }
        assert!(parse_category("not-a-category").is_err());
    }

    #[test]
    fn build_param_def_handles_every_kind() {
        let bool_param = ManifestParam {
            key: "on".into(),
            display_name: "On".into(),
            kind: "bool".into(),
            min: None,
            max: None,
            step: None,
            options: None,
            default: toml::Value::Boolean(true),
        };
        assert!(matches!(
            build_param_def(&bool_param).unwrap().kind,
            ParamKind::Bool { default: true }
        ));

        let int_param = ManifestParam {
            key: "n".into(),
            display_name: "N".into(),
            kind: "int".into(),
            min: Some(0.0),
            max: Some(10.0),
            step: Some(1.0),
            options: None,
            default: toml::Value::Integer(3),
        };
        assert!(matches!(
            build_param_def(&int_param).unwrap().kind,
            ParamKind::IntRange {
                min: 0,
                max: 10,
                step: 1,
                default: 3
            }
        ));

        let choice_param = ManifestParam {
            key: "mode".into(),
            display_name: "Mode".into(),
            kind: "choice".into(),
            min: None,
            max: None,
            step: None,
            options: Some(vec!["a".into(), "b".into()]),
            default: toml::Value::String("a".into()),
        };
        let ParamKind::Choice { options, default } = build_param_def(&choice_param).unwrap().kind
        else {
            panic!("expected Choice");
        };
        assert_eq!(options, &["a", "b"]);
        assert_eq!(default, "a");
    }

    #[test]
    fn build_param_def_rejects_a_kind_missing_its_required_fields() {
        let broken = ManifestParam {
            key: "n".into(),
            display_name: "N".into(),
            kind: "int".into(),
            min: None, // missing
            max: Some(10.0),
            step: None,
            options: None,
            default: toml::Value::Integer(3),
        };
        assert!(build_param_def(&broken).is_err());
    }

    #[test]
    fn load_plugin_reports_a_clean_error_for_a_missing_manifest() {
        let result = load_plugin(Path::new("/nonexistent/plugin.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn load_plugins_dir_on_a_missing_directory_returns_empty_without_panicking() {
        let plugins = load_plugins_dir(Path::new("/nonexistent/plugins/dir"));
        assert!(plugins.is_empty());
    }
}
