use crate::image::WorkingImage;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum EffectError {
    #[error("invalid parameter '{0}'")]
    InvalidParam(String),
    #[error("unknown effect '{0}'")]
    UnknownEffect(String),
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

/// A [`ParamDef`]'s type, range and default, kept to primitive types so schemas can
/// be plain `const`/`static` data — the actual runtime values in a [`ParamValues`]
/// map are still full [`Value`]s, since those come from JSON (UI, recipes,
/// plugins). Carrying `min`/`max`/`step` lets the UI generate slider controls
/// directly from the schema instead of hardcoding per-effect knowledge.
#[derive(Debug, Clone, Copy)]
pub enum ParamKind {
    Bool {
        default: bool,
    },
    IntRange {
        min: i64,
        max: i64,
        step: i64,
        default: i64,
    },
    FloatRange {
        min: f64,
        max: f64,
        step: f64,
        default: f64,
    },
    /// A fixed set of named options (rendered as a `<select>`), e.g. picking
    /// which built-in palette to map an image against.
    Choice {
        options: &'static [&'static str],
        default: &'static str,
    },
}

/// Describes one parameter an [`Effect`] accepts, used both to auto-generate UI
/// controls and to validate recipe/plugin input.
#[derive(Debug, Clone, Copy)]
pub struct ParamDef {
    pub key: &'static str,
    pub display_name: &'static str,
    pub kind: ParamKind,
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
