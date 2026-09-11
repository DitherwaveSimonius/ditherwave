use crate::effect::ParamValues;
use serde::{Deserialize, Serialize};

/// One entry in an [`EffectStack`]: which effect (by registry key), its parameters,
/// and whether it's currently active. This is the shape persisted in recipes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectNode {
    pub id: String,
    pub effect_key: String,
    pub params: ParamValues,
    pub enabled: bool,
}

/// An ordered, user-editable list of effects applied top to bottom — deliberately a
/// linear list rather than a DAG, to match the "simple, small UI" goal.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EffectStack {
    pub nodes: Vec<EffectNode>,
}
