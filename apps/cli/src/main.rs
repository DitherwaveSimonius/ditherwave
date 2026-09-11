use clap::Parser;
use ditherwave_core::{io, Registry, WorkingImage};
use ditherwave_recipe::Recipe;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::path::{Path, PathBuf};

/// Batch-apply a ditherwave recipe (built in the desktop app, saved as TOML) to
/// one photo or every supported photo in a folder.
#[derive(Parser)]
#[command(name = "ditherwave", version, about)]
struct Cli {
    /// Path to a recipe TOML file (exported from the desktop app).
    #[arg(long)]
    recipe: PathBuf,

    /// A single image/RAW file, or a folder to process every supported file in.
    #[arg(long)]
    input: PathBuf,

    /// Folder to write output PNGs into (created if it doesn't exist).
    #[arg(long)]
    output: PathBuf,
}

const RASTER_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg"];

fn is_supported_input(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match ext {
        Some(ext) => {
            RASTER_EXTENSIONS.contains(&ext.as_str()) || ditherwave_raw::is_raw_extension(path)
        }
        None => false,
    }
}

fn open_any(path: &Path) -> Result<WorkingImage, String> {
    if ditherwave_raw::is_raw_extension(path) {
        ditherwave_raw::open_raw(path).map_err(|e| e.to_string())
    } else {
        io::open_raster(path).map_err(|e| e.to_string())
    }
}

fn collect_inputs(input: &Path) -> Result<Vec<PathBuf>, String> {
    if input.is_dir() {
        let mut files: Vec<PathBuf> = std::fs::read_dir(input)
            .map_err(|e| format!("failed to read '{}': {e}", input.display()))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.is_file() && is_supported_input(path))
            .collect();
        files.sort();
        Ok(files)
    } else if input.is_file() {
        Ok(vec![input.to_path_buf()])
    } else {
        Err(format!(
            "'{}' is neither a file nor a directory",
            input.display()
        ))
    }
}

fn process_one(
    path: &Path,
    stack: &ditherwave_core::EffectStack,
    registry: &Registry,
    output_dir: &Path,
) -> Result<(), String> {
    let image = open_any(path)?;
    let rendered = stack.run(&image, registry, 0).map_err(|e| e.to_string())?;
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    let out_path = output_dir.join(format!("{stem}.png"));
    io::save_raster(&rendered, &out_path).map_err(|e| e.to_string())
}

/// Built-in effects plus any WASM plugins found in the user's plugins
/// directory (same location the desktop app loads from), so a recipe using
/// a plugin effect works identically via the CLI. A plugin that fails to
/// load is skipped (logged to stderr) rather than aborting the whole run.
fn build_registry() -> Registry {
    let mut registry = Registry::with_builtins();
    if let Some(dir) = ditherwave_plugin_host::default_plugins_dir() {
        for plugin in ditherwave_plugin_host::load_plugins_dir(&dir) {
            registry.register(plugin.key(), plugin);
        }
    }
    registry
}

fn main() {
    let cli = Cli::parse();

    let recipe = match Recipe::load(&cli.recipe) {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "error: failed to load recipe '{}': {e}",
                cli.recipe.display()
            );
            std::process::exit(1);
        }
    };
    let stack = recipe.to_stack();

    let inputs = match collect_inputs(&cli.input) {
        Ok(files) if files.is_empty() => {
            eprintln!(
                "error: no supported images found at '{}'",
                cli.input.display()
            );
            std::process::exit(1);
        }
        Ok(files) => files,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = std::fs::create_dir_all(&cli.output) {
        eprintln!(
            "error: failed to create output directory '{}': {e}",
            cli.output.display()
        );
        std::process::exit(1);
    }

    let registry = build_registry();
    let bar = ProgressBar::new(inputs.len() as u64);
    bar.set_style(
        ProgressStyle::with_template("{bar:40.cyan/blue} {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("##-"),
    );

    let failures: Vec<(PathBuf, String)> = inputs
        .par_iter()
        .map(|path| {
            bar.set_message(
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            );
            let result = process_one(path, &stack, &registry, &cli.output);
            bar.inc(1);
            (path.clone(), result)
        })
        .filter_map(|(path, result)| result.err().map(|e| (path, e)))
        .collect();

    bar.finish_and_clear();

    for (path, error) in &failures {
        eprintln!("error processing '{}': {error}", path.display());
    }
    println!(
        "Processed {} file(s) with recipe '{}': {} succeeded, {} failed.",
        inputs.len(),
        recipe.meta.name,
        inputs.len() - failures.len(),
        failures.len()
    );

    if !failures.is_empty() {
        std::process::exit(1);
    }
}
