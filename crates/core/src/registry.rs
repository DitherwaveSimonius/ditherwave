use crate::effect::Effect;
use std::collections::HashMap;

/// Maps an [`Effect::key`] to a constructor, so effects can be looked up by name
/// both when building the "add effect" UI and when reconstructing a stack from a
/// serialized recipe.
#[derive(Default)]
pub struct Registry {
    constructors: HashMap<&'static str, fn() -> Box<dyn Effect>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, key: &'static str, ctor: fn() -> Box<dyn Effect>) {
        self.constructors.insert(key, ctor);
    }

    pub fn create(&self, key: &str) -> Option<Box<dyn Effect>> {
        self.constructors.get(key).map(|ctor| ctor())
    }

    pub fn keys(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.constructors.keys().copied()
    }
}
