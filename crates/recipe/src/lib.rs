//! A "recipe" is a saved [`EffectStack`], serialized as human-editable TOML.
//! The GUI exports one from whatever stack a user built by hand; the CLI
//! loads the same file to replay that exact look across a whole folder.
//! This is the one translation point between the two — both ends of a
//! recipe file agree only on this shape, nothing about how it was produced
//! or will be consumed.

use ditherwave_core::{EffectNode, EffectStack, ParamValues};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum RecipeError {
    #[error("failed to parse recipe: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("failed to serialize recipe: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeMeta {
    pub name: String,
    #[serde(default)]
    pub created_with: String,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeEffect {
    pub key: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub params: ParamValues,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub meta: RecipeMeta,
    #[serde(default)]
    pub effects: Vec<RecipeEffect>,
}

impl Recipe {
    pub fn from_stack(name: impl Into<String>, stack: &EffectStack) -> Self {
        Self {
            meta: RecipeMeta {
                name: name.into(),
                created_with: format!("ditherwave-recipe {}", env!("CARGO_PKG_VERSION")),
            },
            effects: stack
                .nodes
                .iter()
                .map(|n| RecipeEffect {
                    key: n.effect_key.clone(),
                    enabled: n.enabled,
                    params: n.params.clone(),
                })
                .collect(),
        }
    }

    /// Reconstructs an [`EffectStack`] from this recipe. Node ids are freshly
    /// generated (their position in the file) since a recipe doesn't need
    /// the stable per-node identity a live, user-editable stack does.
    pub fn to_stack(&self) -> EffectStack {
        EffectStack {
            nodes: self
                .effects
                .iter()
                .enumerate()
                .map(|(i, e)| EffectNode {
                    id: i.to_string(),
                    effect_key: e.key.clone(),
                    params: e.params.clone(),
                    enabled: e.enabled,
                })
                .collect(),
        }
    }

    pub fn from_toml_str(s: &str) -> Result<Self, RecipeError> {
        Ok(toml::from_str(s)?)
    }

    pub fn to_toml_string(&self) -> Result<String, RecipeError> {
        Ok(toml::to_string_pretty(self)?)
    }

    pub fn load(path: &Path) -> Result<Self, RecipeError> {
        Self::from_toml_str(&std::fs::read_to_string(path)?)
    }

    pub fn save(&self, path: &Path) -> Result<(), RecipeError> {
        Ok(std::fs::write(path, self.to_toml_string()?)?)
    }
}

#[cfg(test)]
fn stack_with_one_node(key: &str, params: ParamValues) -> EffectStack {
    EffectStack {
        nodes: vec![EffectNode {
            id: "0".into(),
            effect_key: key.into(),
            params,
            enabled: true,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_toml() {
        let mut params = ParamValues::new();
        params.insert("levels".into(), serde_json::json!(4));
        params.insert("serpentine".into(), serde_json::json!(true));
        let stack = stack_with_one_node("error_diffusion.floyd_steinberg", params);

        let recipe = Recipe::from_stack("test recipe", &stack);
        let toml_str = recipe.to_toml_string().unwrap();
        let parsed = Recipe::from_toml_str(&toml_str).unwrap();

        assert_eq!(parsed.meta.name, "test recipe");
        assert_eq!(parsed.effects.len(), 1);
        assert_eq!(parsed.effects[0].key, "error_diffusion.floyd_steinberg");
        assert!(parsed.effects[0].enabled);
        assert_eq!(
            parsed.effects[0].params.get("levels"),
            Some(&serde_json::json!(4))
        );
    }

    #[test]
    fn to_stack_reconstructs_an_equivalent_effect_stack() {
        let mut params = ParamValues::new();
        params.insert("threshold".into(), serde_json::json!(0.3));
        let original = stack_with_one_node("pattern.threshold", params);

        let recipe = Recipe::from_stack("r", &original);
        let rebuilt = recipe.to_stack();

        assert_eq!(rebuilt.nodes.len(), original.nodes.len());
        assert_eq!(rebuilt.nodes[0].effect_key, original.nodes[0].effect_key);
        assert_eq!(rebuilt.nodes[0].params, original.nodes[0].params);
        assert_eq!(rebuilt.nodes[0].enabled, original.nodes[0].enabled);
    }

    #[test]
    fn disabled_and_missing_fields_default_sensibly() {
        let toml_str = r#"
            [meta]
            name = "minimal"

            [[effects]]
            key = "pattern.threshold"
        "#;
        let recipe = Recipe::from_toml_str(toml_str).unwrap();
        assert_eq!(recipe.meta.created_with, "");
        assert!(recipe.effects[0].enabled);
        assert!(recipe.effects[0].params.is_empty());
    }

    #[test]
    fn produces_the_documented_array_of_tables_shape() {
        // Locks in the file shape the project plan documented, so the format
        // doesn't silently drift into something less hand-editable.
        let mut params = ParamValues::new();
        params.insert("palette".into(), serde_json::json!("gameboy"));
        let stack = stack_with_one_node("color.palette_map", params);
        let toml_str = Recipe::from_stack("shape-check", &stack)
            .to_toml_string()
            .unwrap();

        assert!(toml_str.contains("[[effects]]"));
        assert!(toml_str.contains("key = \"color.palette_map\""));
        assert!(toml_str.contains("[effects.params]"));
    }

    #[test]
    fn rejects_malformed_toml() {
        assert!(Recipe::from_toml_str("this is not valid toml [[[").is_err());
    }
}
