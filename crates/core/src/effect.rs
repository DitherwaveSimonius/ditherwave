use crate::image::WorkingImage;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum EffectError {
    #[error("invalid parameter '{0}'")]
    InvalidParam(String),
    #[error("cancelled")]
    Cancelled,
}

pub type EffectResult<T> = Result<T, EffectError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectCategory {
    ErrorDiffusion,
    Ordered,
    Pattern,
    Glitch,
    Special,
    Color,
}

/// Describes one parameter an [`Effect`] accepts, used both to auto-generate UI
/// controls and to validate recipe/plugin input.
#[derive(Debug, Clone)]
pub struct ParamDef {
    pub key: &'static str,
    pub display_name: &'static str,
    pub default: Value,
}

/// Concrete parameter values for one [`EffectNode`], keyed by [`ParamDef::key`].
pub type ParamValues = HashMap<String, Value>;

pub struct EffectContext<'a> {
    pub rng_seed: u64,
    /// True when rendering the downsampled live preview rather than a full export,
    /// letting an effect pick a cheaper approximation.
    pub preview: bool,
    pub cancelled: &'a dyn Fn() -> bool,
}

/// Shared contract every dithering/glitch/special algorithm implements, whether
/// built in or loaded from a WASM plugin. Implementations must be stateless and
/// safe to call from any thread so the stack can cache and parallelize freely.
pub trait Effect: Send + Sync {
    /// Stable identifier used in the [`Registry`](crate::registry::Registry) and in
    /// serialized recipes — must never change once released.
    fn key(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn category(&self) -> EffectCategory;
    fn params_schema(&self) -> &'static [ParamDef];
    fn apply(
        &self,
        input: &WorkingImage,
        params: &ParamValues,
        ctx: &EffectContext,
    ) -> EffectResult<WorkingImage>;
}
