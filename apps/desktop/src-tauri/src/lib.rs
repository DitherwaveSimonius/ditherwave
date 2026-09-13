use base64::Engine;
use ditherwave_core::{
    io, EffectContext, EffectNode, EffectStack, ParamKind, Registry, WorkingImage,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Mutex;

/// One rendered stage of the stack, cached alongside the hash of the node that
/// produced it so a later render can tell whether it's still valid.
struct CachedStage {
    node_hash: u64,
    image: WorkingImage,
}

struct AppState {
    registry: Registry,
    original: Mutex<Option<WorkingImage>>,
    stack: Mutex<EffectStack>,
    /// Parallel to `stack`'s enabled nodes, in order. Re-rendering reuses the
    /// cached image for every node from the top until the first one whose
    /// (effect_key, params) hash no longer matches — matching the "editing
    /// node N only invalidates N..end" design from the project's plan.
    cache: Mutex<Vec<CachedStage>>,
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
    /// "bool" | "int" | "float" | "choice" — tells the UI which control to render.
    kind: &'static str,
    min: f64,
    max: f64,
    step: f64,
    options: &'static [&'static str],
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

fn category_name(category: ditherwave_core::EffectCategory) -> &'static str {
    use ditherwave_core::EffectCategory::*;
    match category {
        ErrorDiffusion => "ErrorDiffusion",
        Ordered => "Ordered",
        Pattern => "Pattern",
        Glitch => "Glitch",
        Special => "Special",
        Color => "Color",
    }
}

fn hash_node(node: &EffectNode) -> u64 {
    let mut hasher = DefaultHasher::new();
    node.effect_key.hash(&mut hasher);
    // HashMap iteration order isn't stable, so hash params via their sorted
    // JSON representation instead of hashing the map directly.
    let mut params: Vec<(&String, &Value)> = node.params.iter().collect();
    params.sort_by_key(|(k, _)| k.as_str());
    for (k, v) in params {
        k.hash(&mut hasher);
        v.to_string().hash(&mut hasher);
    }
    hasher.finish()
}

/// Runs `stack` against `original`, reusing `cache` for every node up to the
/// first change, and leaves `cache` updated to match the new result.
fn render_stack(
    original: &WorkingImage,
    stack: &EffectStack,
    registry: &Registry,
    cache: &mut Vec<CachedStage>,
) -> Result<WorkingImage, String> {
    let mut current = original.clone();
    let mut new_cache = Vec::new();
    let mut cache_iter = std::mem::take(cache).into_iter();
    let mut still_valid = true;
    let no_cancel = || false;

    for node in stack.nodes.iter().filter(|n| n.enabled) {
        let node_hash = hash_node(node);

        if still_valid {
            match cache_iter.next() {
                Some(stage) if stage.node_hash == node_hash => {
                    current = stage.image.clone();
                    new_cache.push(stage);
                    continue;
                }
                _ => still_valid = false,
            }
        }

        let effect = registry
            .get(&node.effect_key)
            .ok_or_else(|| format!("unknown effect '{}'", node.effect_key))?;
        let ctx = EffectContext {
            rng_seed: 0,
            preview: false,
            cancelled: &no_cancel,
        };
        current = effect
            .apply(&current, &node.params, &ctx)
            .map_err(|e| e.to_string())?;
        new_cache.push(CachedStage {
            node_hash,
            image: current.clone(),
        });
    }

    *cache = new_cache;
    Ok(current)
}

#[tauri::command]
fn list_effects(state: tauri::State<AppState>) -> Vec<EffectDescriptor> {
    state
        .registry
        .keys()
        .filter_map(|key| state.registry.get(key))
        .map(|effect| EffectDescriptor {
            key: effect.key(),
            display_name: effect.display_name(),
            category: category_name(effect.category()),
            params: effect
                .params_schema()
                .iter()
                .map(|p| {
                    let (kind, min, max, step, options, default) = match p.kind {
                        ParamKind::Bool { default } => {
                            ("bool", 0.0, 1.0, 1.0, [].as_slice(), Value::from(default))
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
                            [].as_slice(),
                            Value::from(default),
                        ),
                        ParamKind::FloatRange {
                            min,
                            max,
                            step,
                            default,
                        } => ("float", min, max, step, [].as_slice(), Value::from(default)),
                        ParamKind::Choice { options, default } => {
                            ("choice", 0.0, 0.0, 0.0, options, Value::from(default))
                        }
                    };
                    ParamDescriptor {
                        key: p.key,
                        display_name: p.display_name,
                        kind,
                        min,
                        max,
                        step,
                        options,
                        default,
                    }
                })
                .collect(),
        })
        .collect()
}

/// Bundled into the binary at compile time so the app has something to show
/// before the user opens their own photo — see branding/generate-demo for
/// how it was produced.
const DEMO_IMAGE: &[u8] = include_bytes!("../assets/demo.png");

#[tauri::command]
fn load_demo_image(state: tauri::State<AppState>) -> Result<ImagePayload, String> {
    let working = io::open_raster_bytes(DEMO_IMAGE).map_err(|e| e.to_string())?;
    let payload = to_payload(&working)?;
    *state.original.lock().unwrap() = Some(working);
    *state.stack.lock().unwrap() = EffectStack::default();
    state.cache.lock().unwrap().clear();
    Ok(payload)
}

#[tauri::command]
fn open_image(path: String, state: tauri::State<AppState>) -> Result<ImagePayload, String> {
    let path = Path::new(&path);
    let is_svg = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"));
    let working = if is_svg {
        ditherwave_vector::open_svg(path, None).map_err(|e| e.to_string())?
    } else if ditherwave_raw::is_raw_extension(path) {
        ditherwave_raw::open_raw(path).map_err(|e| e.to_string())?
    } else {
        io::open_raster(path).map_err(|e| e.to_string())?
    };
    let payload = to_payload(&working)?;
    *state.original.lock().unwrap() = Some(working);
    *state.stack.lock().unwrap() = EffectStack::default();
    state.cache.lock().unwrap().clear();
    Ok(payload)
}

#[tauri::command]
fn set_stack(
    nodes: Vec<EffectNode>,
    state: tauri::State<AppState>,
) -> Result<ImagePayload, String> {
    let original = state.original.lock().unwrap();
    let original = original.as_ref().ok_or("no image open")?;

    let new_stack = EffectStack { nodes };
    let mut cache = state.cache.lock().unwrap();
    let rendered = render_stack(original, &new_stack, &state.registry, &mut cache)?;
    *state.stack.lock().unwrap() = new_stack;
    to_payload(&rendered)
}

#[tauri::command]
fn export_image(path: String, state: tauri::State<AppState>) -> Result<(), String> {
    let original = state.original.lock().unwrap();
    let original = original.as_ref().ok_or("no image to export")?;
    let stack = state.stack.lock().unwrap();
    let mut cache = state.cache.lock().unwrap();
    let rendered = render_stack(original, &stack, &state.registry, &mut cache)?;
    io::save_raster(&rendered, Path::new(&path)).map_err(|e| e.to_string())
}

#[tauri::command]
fn export_dots_svg(path: String, state: tauri::State<AppState>) -> Result<(), String> {
    let original = state.original.lock().unwrap();
    let original = original.as_ref().ok_or("no image to export")?;
    let stack = state.stack.lock().unwrap();
    let mut cache = state.cache.lock().unwrap();
    let rendered = render_stack(original, &stack, &state.registry, &mut cache)?;
    ditherwave_vector::export_dots(&rendered, Path::new(&path), 0.45, [1.0, 1.0, 1.0])
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn export_recipe(path: String, state: tauri::State<AppState>) -> Result<(), String> {
    let stack = state.stack.lock().unwrap();
    let name = Path::new(&path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "ditherwave recipe".to_string());
    let recipe = ditherwave_recipe::Recipe::from_stack(name, &stack);
    recipe.save(Path::new(&path)).map_err(|e| e.to_string())
}

/// Response to loading a recipe: the desktop UI needs both the reconstructed
/// node list (to repopulate the "Effects" panel) and a rendered preview.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StackPayload {
    nodes: Vec<EffectNode>,
    image: ImagePayload,
}

#[tauri::command]
fn import_recipe(path: String, state: tauri::State<AppState>) -> Result<StackPayload, String> {
    let recipe = ditherwave_recipe::Recipe::load(Path::new(&path)).map_err(|e| e.to_string())?;
    let new_stack = recipe.to_stack();

    let original = state.original.lock().unwrap();
    let original = original.as_ref().ok_or("no image open")?;
    let mut cache = state.cache.lock().unwrap();
    let rendered = render_stack(original, &new_stack, &state.registry, &mut cache)?;
    let image = to_payload(&rendered)?;
    let nodes = new_stack.nodes.clone();
    *state.stack.lock().unwrap() = new_stack;

    Ok(StackPayload { nodes, image })
}

/// Built-in effects plus any WASM plugins found in the user's plugins
/// directory (`~/Library/Application Support/ditherwave/plugins` on macOS).
/// A plugin that fails to load is skipped (logged to stderr by
/// `load_plugins_dir`) rather than failing app startup.
fn build_registry() -> Registry {
    let mut registry = Registry::with_builtins();
    if let Some(dir) = ditherwave_plugin_host::default_plugins_dir() {
        for plugin in ditherwave_plugin_host::load_plugins_dir(&dir) {
            registry.register(plugin.key(), plugin);
        }
    }
    registry
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            registry: build_registry(),
            original: Mutex::new(None),
            stack: Mutex::new(EffectStack::default()),
            cache: Mutex::new(Vec::new()),
        })
        .invoke_handler(tauri::generate_handler![
            list_effects,
            load_demo_image,
            open_image,
            set_stack,
            export_image,
            export_dots_svg,
            export_recipe,
            import_recipe
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

#[cfg(test)]
mod tests {
    use super::*;
    use ditherwave_core::{
        ColorSpace, Effect, EffectCategory, EffectResult, ParamDef, ParamValues,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    static CALLS_A: AtomicUsize = AtomicUsize::new(0);
    static CALLS_B: AtomicUsize = AtomicUsize::new(0);

    struct CountingA;
    impl Effect for CountingA {
        fn key(&self) -> &'static str {
            "test.a"
        }
        fn display_name(&self) -> &'static str {
            "A"
        }
        fn category(&self) -> EffectCategory {
            EffectCategory::Special
        }
        fn params_schema(&self) -> &'static [ParamDef] {
            &[]
        }
        fn apply(
            &self,
            input: &WorkingImage,
            _params: &ParamValues,
            _ctx: &EffectContext,
        ) -> EffectResult<WorkingImage> {
            CALLS_A.fetch_add(1, Ordering::SeqCst);
            Ok(input.clone())
        }
    }

    struct CountingB;
    impl Effect for CountingB {
        fn key(&self) -> &'static str {
            "test.b"
        }
        fn display_name(&self) -> &'static str {
            "B"
        }
        fn category(&self) -> EffectCategory {
            EffectCategory::Special
        }
        fn params_schema(&self) -> &'static [ParamDef] {
            &[]
        }
        fn apply(
            &self,
            input: &WorkingImage,
            _params: &ParamValues,
            _ctx: &EffectContext,
        ) -> EffectResult<WorkingImage> {
            CALLS_B.fetch_add(1, Ordering::SeqCst);
            Ok(input.clone())
        }
    }

    fn counting_registry() -> Registry {
        let mut registry = Registry::new();
        registry.register("test.a", Box::new(CountingA));
        registry.register("test.b", Box::new(CountingB));
        registry
    }

    fn node(id: &str, key: &str) -> EffectNode {
        EffectNode {
            id: id.into(),
            effect_key: key.into(),
            params: Default::default(),
            enabled: true,
        }
    }

    #[test]
    fn hash_node_ignores_param_insertion_order() {
        let mut a = node("1", "x");
        a.params.insert("z".into(), Value::from(1));
        a.params.insert("a".into(), Value::from(2));

        let mut b = node("1", "x");
        b.params.insert("a".into(), Value::from(2));
        b.params.insert("z".into(), Value::from(1));

        assert_eq!(hash_node(&a), hash_node(&b));
    }

    #[test]
    fn hash_node_differs_when_a_param_value_changes() {
        let mut a = node("1", "x");
        a.params.insert("levels".into(), Value::from(2));
        let mut b = node("1", "x");
        b.params.insert("levels".into(), Value::from(4));

        assert_ne!(hash_node(&a), hash_node(&b));
    }

    #[test]
    fn render_stack_reuses_cached_nodes_up_to_the_first_change() {
        let registry = counting_registry();
        let image = WorkingImage::new(2, 2, ColorSpace::Srgb);
        let stack = EffectStack {
            nodes: vec![node("1", "test.a"), node("2", "test.b")],
        };
        let mut cache = Vec::new();

        render_stack(&image, &stack, &registry, &mut cache).unwrap();
        let (a_after_first, b_after_first) = (
            CALLS_A.load(Ordering::SeqCst),
            CALLS_B.load(Ordering::SeqCst),
        );
        assert_eq!(cache.len(), 2);

        // Re-rendering the identical stack must hit the cache for both nodes.
        render_stack(&image, &stack, &registry, &mut cache).unwrap();
        assert_eq!(CALLS_A.load(Ordering::SeqCst), a_after_first);
        assert_eq!(CALLS_B.load(Ordering::SeqCst), b_after_first);

        // Changing the first node's params must invalidate it *and* everything
        // downstream, even though the second node's own params didn't change.
        let mut changed = stack.clone();
        changed.nodes[0].params.insert("x".into(), Value::from(1));
        render_stack(&image, &changed, &registry, &mut cache).unwrap();
        assert_eq!(CALLS_A.load(Ordering::SeqCst), a_after_first + 1);
        assert_eq!(CALLS_B.load(Ordering::SeqCst), b_after_first + 1);
    }

    #[test]
    fn render_stack_skips_disabled_nodes() {
        let registry = counting_registry();
        let image = WorkingImage::new(2, 2, ColorSpace::Srgb);
        let mut disabled_b = node("2", "test.b");
        disabled_b.enabled = false;
        let stack = EffectStack {
            nodes: vec![node("1", "test.a"), disabled_b],
        };
        let mut cache = Vec::new();

        render_stack(&image, &stack, &registry, &mut cache).unwrap();
        assert_eq!(
            cache.len(),
            1,
            "disabled node should not produce a cache stage"
        );
    }
}
