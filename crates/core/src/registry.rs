use crate::effect::Effect;
use std::collections::HashMap;
use std::sync::Arc;

/// Maps an [`Effect::key`] to a shared, already-constructed instance, so
/// effects can be looked up by name both when building the "add effect" UI
/// and when running a stack. Effects are stored ready-to-use (not as
/// constructors) because not every effect is as cheap to build as a
/// stateless built-in struct — a WASM plugin (`ditherwave-plugin-host`) is a
/// loaded, instantiated module that would be wasteful (or, since it holds a
/// `Mutex`, impossible without `Clone`) to reconstruct on every lookup.
#[derive(Default)]
pub struct Registry {
    effects: HashMap<&'static str, Arc<dyn Effect>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// A registry pre-populated with every algorithm built into `ditherwave-core`.
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        for (key, ctor) in crate::algorithms::builtins() {
            registry.register(key, ctor());
        }
        registry
    }

    pub fn register(&mut self, key: &'static str, effect: Box<dyn Effect>) {
        self.effects.insert(key, Arc::from(effect));
    }

    /// A cheap clone of the shared instance registered under `key`, if any.
    pub fn get(&self, key: &str) -> Option<Arc<dyn Effect>> {
        self.effects.get(key).cloned()
    }

    pub fn keys(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.effects.keys().copied()
    }
}
