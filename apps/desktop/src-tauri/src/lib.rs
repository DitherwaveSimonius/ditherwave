use base64::Engine;
use ditheros_core::{io, EffectContext, ParamKind, Registry, WorkingImage};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

struct AppState {
    registry: Registry,
    original: Mutex<Option<WorkingImage>>,
    processed: Mutex<Option<WorkingImage>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImagePayload {
    width: u32,
    height: u32,
    png_base64: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ParamDescriptor {
    key: &'static str,
    display_name: &'static str,
    /// "bool" | "int" | "float" — tells the UI which control to render.
    kind: &'static str,
    min: f64,
    max: f64,
    step: f64,
    default: Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EffectDescriptor {
    key: &'static str,
    display_name: &'static str,
    category: &'static str,
    params: Vec<ParamDescriptor>,
}

fn to_payload(image: &WorkingImage) -> Result<ImagePayload, String> {
    let png = io::encode_png(image).map_err(|e| e.to_string())?;
    Ok(ImagePayload {
        width: image.width,
        height: image.height,
        png_base64: base64::engine::general_purpose::STANDARD.encode(png),
    })
}

fn category_name(category: ditheros_core::EffectCategory) -> &'static str {
    use ditheros_core::EffectCategory::*;
    match category {
        ErrorDiffusion => "ErrorDiffusion",
        Ordered => "Ordered",
        Pattern => "Pattern",
        Glitch => "Glitch",
        Special => "Special",
        Color => "Color",
    }
}

#[tauri::command]
fn list_effects(state: tauri::State<AppState>) -> Vec<EffectDescriptor> {
    state
        .registry
        .keys()
        .filter_map(|key| state.registry.create(key))
        .map(|effect| EffectDescriptor {
            key: effect.key(),
            display_name: effect.display_name(),
            category: category_name(effect.category()),
            params: effect
                .params_schema()
                .iter()
                .map(|p| {
                    let (kind, min, max, step, default) = match p.kind {
                        ParamKind::Bool { default } => {
                            ("bool", 0.0, 1.0, 1.0, Value::from(default))
                        }
                        ParamKind::IntRange {
                            min,
                            max,
                            step,
                            default,
                        } => (
                            "int",
                            min as f64,
                            max as f64,
                            step as f64,
                            Value::from(default),
                        ),
                        ParamKind::FloatRange {
                            min,
                            max,
                            step,
                            default,
                        } => ("float", min, max, step, Value::from(default)),
                    };
                    ParamDescriptor {
                        key: p.key,
                        display_name: p.display_name,
                        kind,
                        min,
                        max,
                        step,
                        default,
                    }
                })
                .collect(),
        })
        .collect()
}

#[tauri::command]
fn open_image(path: String, state: tauri::State<AppState>) -> Result<ImagePayload, String> {
    let working = io::open_raster(Path::new(&path)).map_err(|e| e.to_string())?;
    let payload = to_payload(&working)?;
    *state.original.lock().unwrap() = Some(working);
    *state.processed.lock().unwrap() = None;
    Ok(payload)
}

#[tauri::command]
fn apply_effect(
    effect_key: String,
    params: HashMap<String, Value>,
    state: tauri::State<AppState>,
) -> Result<ImagePayload, String> {
    let output = {
        let original = state.original.lock().unwrap();
        let input = original.as_ref().ok_or("no image open")?;
        let effect = state
            .registry
            .create(&effect_key)
            .ok_or_else(|| format!("unknown effect '{effect_key}'"))?;
        let no_cancel = || false;
        let ctx = EffectContext {
            rng_seed: 0,
            preview: false,
            cancelled: &no_cancel,
        };
        effect
            .apply(input, &params, &ctx)
            .map_err(|e| e.to_string())?
    };
    let payload = to_payload(&output)?;
    *state.processed.lock().unwrap() = Some(output);
    Ok(payload)
}

#[tauri::command]
fn export_image(path: String, state: tauri::State<AppState>) -> Result<(), String> {
    let processed = state.processed.lock().unwrap();
    let original = state.original.lock().unwrap();
    let image = processed
        .as_ref()
        .or(original.as_ref())
        .ok_or("no image to export")?;
    io::save_raster(image, Path::new(&path)).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            registry: Registry::with_builtins(),
            original: Mutex::new(None),
            processed: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            list_effects,
            open_image,
            apply_effect,
            export_image
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
