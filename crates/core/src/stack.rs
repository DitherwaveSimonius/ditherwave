use crate::effect::{EffectContext, EffectError, ParamValues};
use crate::image::WorkingImage;
use crate::registry::Registry;
use serde::{Deserialize, Serialize};

/// One entry in an [`EffectStack`]: which effect (by registry key), its parameters,
/// and whether it's currently active. This is the shape persisted in recipes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectNode {
    pub id: String,
    pub effect_key: String,
    pub params: ParamValues,
    pub enabled: bool,
}

/// An ordered, user-editable list of effects applied top to bottom — deliberately a
/// linear list rather than a DAG, to match the "simple, small UI" goal.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectStack {
    pub nodes: Vec<EffectNode>,
}

impl EffectStack {
    /// Runs every enabled node top to bottom against `input`, feeding each node's
    /// output into the next. This is the simple, uncached reference
    /// implementation — the desktop app additionally caches per-node results
    /// (see `apps/desktop/src-tauri`), but the CLI/batch path can use this
    /// directly since a one-shot render has nothing to reuse a cache for.
    pub fn run(
        &self,
        input: &WorkingImage,
        registry: &Registry,
        rng_seed: u64,
    ) -> Result<WorkingImage, EffectError> {
        let mut current = input.clone();
        let no_cancel = || false;
        for node in self.nodes.iter().filter(|n| n.enabled) {
            let effect = registry
                .create(&node.effect_key)
                .ok_or_else(|| EffectError::InvalidParam(node.effect_key.clone()))?;
            let ctx = EffectContext {
                rng_seed,
                preview: false,
                cancelled: &no_cancel,
            };
            current = effect.apply(&current, &node.params, &ctx)?;
        }
        Ok(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algorithms::test_util::solid_image;
    use crate::registry::Registry;

    fn node(key: &str, enabled: bool) -> EffectNode {
        EffectNode {
            id: key.into(),
            effect_key: key.into(),
            params: Default::default(),
            enabled,
        }
    }

    #[test]
    fn empty_stack_returns_the_input_unchanged() {
        let input = solid_image(2, 2, 0.5);
        let stack = EffectStack::default();
        let out = stack.run(&input, &Registry::with_builtins(), 0).unwrap();
        assert_eq!(out.pixels, input.pixels);
    }

    #[test]
    fn chains_effects_in_order() {
        // Threshold at the default 0.5 cutoff turns mid-gray fully white, then a
        // second threshold node has nothing left to do — running it through the
        // stack must match manually chaining the two `apply` calls.
        let input = solid_image(2, 2, 0.6);
        let registry = Registry::with_builtins();
        let stack = EffectStack {
            nodes: vec![
                node("pattern.threshold", true),
                node("pattern.threshold", true),
            ],
        };

        let via_stack = stack.run(&input, &registry, 0).unwrap();

        let threshold = registry.create("pattern.threshold").unwrap();
        let no_cancel = || false;
        let ctx = crate::effect::EffectContext {
            rng_seed: 0,
            preview: false,
            cancelled: &no_cancel,
        };
        let manual = threshold
            .apply(
                &threshold.apply(&input, &Default::default(), &ctx).unwrap(),
                &Default::default(),
                &ctx,
            )
            .unwrap();

        assert_eq!(via_stack.pixels, manual.pixels);
    }

    #[test]
    fn skips_disabled_nodes() {
        let input = solid_image(2, 2, 0.6);
        let registry = Registry::with_builtins();
        let stack = EffectStack {
            nodes: vec![node("pattern.threshold", false)],
        };

        let out = stack.run(&input, &registry, 0).unwrap();
        assert_eq!(out.pixels, input.pixels, "disabled node must not run");
    }

    #[test]
    fn unknown_effect_key_is_an_error() {
        let input = solid_image(2, 2, 0.5);
        let stack = EffectStack {
            nodes: vec![node("does.not.exist", true)],
        };
        assert!(stack.run(&input, &Registry::with_builtins(), 0).is_err());
    }
}
