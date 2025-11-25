//! Animation-related types for the IR view schema.
//!
//! These types define how animations are specified in ViewDocuments.
//! They are serialization-focused types that get converted to runtime
//! animation types by the renderer.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Easing function specification for animations.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EasingSpec {
    /// Linear interpolation (no easing).
    Linear,
    /// CSS `ease` - Slow start, fast middle, slow end.
    Ease,
    /// CSS `ease-in` - Slow start, accelerating.
    EaseIn,
    /// CSS `ease-out` - Fast start, decelerating.
    EaseOut,
    /// CSS `ease-in-out` - Slow start and end, fast middle.
    EaseInOut,
    /// Custom cubic bezier curve.
    CubicBezier { x1: f32, y1: f32, x2: f32, y2: f32 },
    /// Stepped animation with discrete jumps.
    Steps { count: u32, position: StepPositionSpec },
}

impl Default for EasingSpec {
    fn default() -> Self {
        Self::Ease
    }
}

/// Position for stepped animations.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StepPositionSpec {
    /// Jump at the start of each interval.
    Start,
    /// Jump at the end of each interval.
    #[default]
    End,
    /// Jump at both start and end.
    Both,
    /// No jump at start or end.
    None,
}

/// How many times an animation should repeat.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IterationCountSpec {
    /// Run the animation a specific number of times (can be fractional).
    Count { count: f32 },
    /// Run the animation indefinitely.
    Infinite,
}

impl Default for IterationCountSpec {
    fn default() -> Self {
        Self::Count { count: 1.0 }
    }
}

/// Direction of animation playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationDirectionSpec {
    /// Play animation forward each iteration.
    #[default]
    Normal,
    /// Play animation backward each iteration.
    Reverse,
    /// Alternate between forward and backward.
    Alternate,
    /// Alternate, starting with backward.
    AlternateReverse,
}

/// What values to apply before/after the animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationFillModeSpec {
    /// Don't apply any values outside the animation.
    #[default]
    None,
    /// Retain the final keyframe values after animation ends.
    Forwards,
    /// Apply the first keyframe values during the delay period.
    Backwards,
    /// Apply both forwards and backwards behavior.
    Both,
}

/// Which property or properties a transition applies to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransitionTargetSpec {
    /// Transition a specific property.
    Property { property: String },
    /// Transition all animatable properties.
    All,
}

impl Default for TransitionTargetSpec {
    fn default() -> Self {
        Self::All
    }
}

/// Specification for a single property transition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransitionSpecDef {
    /// Which property or properties this transition applies to.
    #[serde(default)]
    pub target: TransitionTargetSpec,
    /// Duration of the transition in milliseconds.
    pub duration_ms: f32,
    /// Delay before the transition starts in milliseconds.
    #[serde(default)]
    pub delay_ms: f32,
    /// Easing function for the transition timing.
    #[serde(default)]
    pub easing: EasingSpec,
}

/// A group of transition specifications for multiple properties.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransitionGroupSpec {
    /// Individual transition specifications.
    #[serde(default)]
    pub specs: Vec<TransitionSpecDef>,
}

/// A single keyframe in an animation sequence.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KeyframeSpec {
    /// Position in the animation timeline (0.0 to 1.0).
    pub offset: f32,
    /// Property values at this keyframe as JSON values.
    #[serde(default)]
    pub values: HashMap<String, serde_json::Value>,
    /// Easing function to use when interpolating TO this keyframe.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub easing: Option<EasingSpec>,
}

/// Definition of a keyframe animation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyframeAnimationSpec {
    /// Name of the animation (for referencing in registries).
    pub name: String,
    /// Ordered list of keyframes (sorted by offset).
    #[serde(default)]
    pub keyframes: Vec<KeyframeSpec>,
    /// Total duration of one iteration in milliseconds.
    #[serde(default)]
    pub duration_ms: f32,
    /// Delay before the animation starts in milliseconds.
    #[serde(default)]
    pub delay_ms: f32,
    /// Number of times to repeat the animation.
    #[serde(default)]
    pub iteration_count: IterationCountSpec,
    /// Direction of playback.
    #[serde(default)]
    pub direction: AnimationDirectionSpec,
    /// Fill mode (what values to apply before/after).
    #[serde(default)]
    pub fill_mode: AnimationFillModeSpec,
    /// Default easing function for keyframes without explicit easing.
    #[serde(default)]
    pub default_easing: EasingSpec,
}

/// Reference to a named keyframe animation with optional overrides.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationRefSpec {
    /// Name of the registered animation to use.
    pub name: String,

    /// Override duration in milliseconds.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<f32>,

    /// Override delay in milliseconds.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay_ms: Option<f32>,

    /// Override iteration count.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iteration_count: Option<IterationCountSpec>,

    /// Override direction.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<AnimationDirectionSpec>,

    /// Override fill mode.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_mode: Option<AnimationFillModeSpec>,

    /// Whether the animation should start immediately.
    #[serde(default = "default_autoplay")]
    pub autoplay: bool,
}

fn default_autoplay() -> bool {
    true
}

/// Animation properties that can be attached to an IR node spec.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeAnimationFields {
    /// Transition configuration for property changes.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<TransitionGroupSpec>,

    /// Keyframe animation to apply to this node.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animation: Option<AnimationRefSpec>,
}

impl NodeAnimationFields {
    /// Check if this spec has any animation configuration.
    pub fn is_empty(&self) -> bool {
        self.transition.is_none() && self.animation.is_none()
    }
}

// ============================================================================
// Transform Types
// ============================================================================

/// 2D transform specification for IR nodes.
///
/// Supports CSS-like transform functions: translate, scale, rotate, skew.
/// Transforms are applied in the order: translate -> rotate -> scale -> skew.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TransformSpec {
    /// Horizontal translation in pixels.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translate_x: Option<f64>,

    /// Vertical translation in pixels.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translate_y: Option<f64>,

    /// Horizontal scale factor (1.0 = no scale).
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale_x: Option<f64>,

    /// Vertical scale factor (1.0 = no scale).
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale_y: Option<f64>,

    /// Rotation angle in degrees.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotate: Option<f64>,

    /// Horizontal skew angle in degrees.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skew_x: Option<f64>,

    /// Vertical skew angle in degrees.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skew_y: Option<f64>,

    /// Transform origin specification.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<TransformOriginSpec>,
}

impl TransformSpec {
    /// Check if this transform has any non-default values.
    pub fn is_identity(&self) -> bool {
        self.translate_x.is_none()
            && self.translate_y.is_none()
            && self.scale_x.is_none()
            && self.scale_y.is_none()
            && self.rotate.is_none()
            && self.skew_x.is_none()
            && self.skew_y.is_none()
    }
}

/// Transform origin specification.
///
/// Determines the point around which transforms are applied.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransformOriginSpec {
    /// Origin at absolute pixel coordinates.
    Absolute { x: f64, y: f64 },
    /// Origin at percentage of element size (0.0-1.0 range, default 0.5, 0.5 = center).
    Percentage { x: f64, y: f64 },
    /// Named origin positions.
    Named { position: NamedOriginSpec },
}

impl Default for TransformOriginSpec {
    fn default() -> Self {
        Self::Named {
            position: NamedOriginSpec::Center,
        }
    }
}

/// Named transform origin positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NamedOriginSpec {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    #[default]
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

// ============================================================================
// Visibility Types
// ============================================================================

/// Visibility state specification for IR nodes.
///
/// Controls whether an element is visible and how it participates in layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisibilitySpec {
    /// Element is visible and participates in layout (default).
    #[default]
    Visible,
    /// Element is invisible but still participates in layout (like CSS visibility: hidden).
    Hidden,
    /// Element is completely removed from layout (like CSS display: none).
    Collapsed,
}
