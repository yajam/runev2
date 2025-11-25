//! Core animation types and data structures.
//!
//! This module defines the fundamental types for the animation system:
//! - `AnimatableValue`: Enum for all animatable property values
//! - `AnimatableProperty`: Enum mapping to IR node properties
//! - `AnimationId`: Unique identifier for animations
//! - `AnimationState`: Current state of an animation

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// Unique identifier for an animation instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AnimationId(pub u64);

impl AnimationId {
    /// Generate a new unique animation ID.
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for AnimationId {
    fn default() -> Self {
        Self::new()
    }
}

/// Current state of an animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationState {
    /// Animation has been created but not yet started (waiting for delay).
    Pending,
    /// Animation is actively running.
    Running,
    /// Animation has been paused.
    Paused,
    /// Animation has completed normally.
    Finished,
    /// Animation was cancelled before completion.
    Cancelled,
}

impl Default for AnimationState {
    fn default() -> Self {
        Self::Pending
    }
}

/// Visibility state for elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    /// Element is visible and participates in layout.
    Visible,
    /// Element is invisible but still participates in layout (like CSS visibility: hidden).
    Hidden,
    /// Element is completely removed from layout (like CSS display: none).
    Collapsed,
}

impl Default for Visibility {
    fn default() -> Self {
        Self::Visible
    }
}

/// Edge insets for padding/margin animation.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct AnimatableEdgeInsets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl AnimatableEdgeInsets {
    pub fn uniform(value: f64) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
}

impl From<rune_ir::view::EdgeInsets> for AnimatableEdgeInsets {
    fn from(e: rune_ir::view::EdgeInsets) -> Self {
        Self {
            top: e.top,
            right: e.right,
            bottom: e.bottom,
            left: e.left,
        }
    }
}

impl From<AnimatableEdgeInsets> for rune_ir::view::EdgeInsets {
    fn from(e: AnimatableEdgeInsets) -> Self {
        Self {
            top: e.top,
            right: e.right,
            bottom: e.bottom,
            left: e.left,
        }
    }
}

/// 2D transform for position, scale, and rotation animation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AnimatableTransform {
    pub translate_x: f64,
    pub translate_y: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    /// Rotation in radians.
    pub rotate: f64,
}

impl Default for AnimatableTransform {
    fn default() -> Self {
        Self {
            translate_x: 0.0,
            translate_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotate: 0.0,
        }
    }
}

impl AnimatableTransform {
    /// Create an AnimatableTransform from an IR TransformSpec.
    ///
    /// Converts degrees to radians for rotation.
    pub fn from_spec(spec: &rune_ir::view::TransformSpec) -> Self {
        Self {
            translate_x: spec.translate_x.unwrap_or(0.0),
            translate_y: spec.translate_y.unwrap_or(0.0),
            scale_x: spec.scale_x.unwrap_or(1.0),
            scale_y: spec.scale_y.unwrap_or(1.0),
            rotate: spec.rotate.unwrap_or(0.0).to_radians(), // Convert degrees to radians
        }
    }
}

/// Box shadow properties for animation.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct AnimatableBoxShadow {
    pub offset_x: f64,
    pub offset_y: f64,
    pub blur: f64,
    /// RGBA color components.
    pub color: [f32; 4],
}

impl AnimatableBoxShadow {
    /// Create a new box shadow with the given parameters.
    pub fn new(offset_x: f64, offset_y: f64, blur: f64, color: [f32; 4]) -> Self {
        Self {
            offset_x,
            offset_y,
            blur,
            color,
        }
    }
}

/// Enum representing all animatable value types.
///
/// This enum wraps the different types of values that can be animated,
/// allowing the animation system to handle them uniformly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnimatableValue {
    /// Numeric value (width, height, opacity, font_size, etc.)
    F64 { value: f64 },
    /// RGBA color value in linear premultiplied space.
    Color { rgba: [f32; 4] },
    /// Edge insets for padding/margin.
    EdgeInsets {
        #[serde(flatten)]
        insets: AnimatableEdgeInsets,
    },
    /// 2D transform (translate, scale, rotate).
    Transform {
        #[serde(flatten)]
        transform: AnimatableTransform,
    },
    /// Box shadow parameters.
    BoxShadow {
        #[serde(flatten)]
        shadow: AnimatableBoxShadow,
    },
    /// Visibility state (visible, hidden, collapsed).
    Visibility { value: Visibility },
}

impl AnimatableValue {
    /// Try to extract an f64 value.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::F64 { value } => Some(*value),
            _ => None,
        }
    }

    /// Try to extract a color value.
    pub fn as_color(&self) -> Option<[f32; 4]> {
        match self {
            Self::Color { rgba } => Some(*rgba),
            _ => None,
        }
    }

    /// Try to extract edge insets.
    pub fn as_edge_insets(&self) -> Option<AnimatableEdgeInsets> {
        match self {
            Self::EdgeInsets { insets } => Some(*insets),
            _ => None,
        }
    }

    /// Try to extract a transform.
    pub fn as_transform(&self) -> Option<AnimatableTransform> {
        match self {
            Self::Transform { transform } => Some(*transform),
            _ => None,
        }
    }

    /// Try to extract box shadow.
    pub fn as_box_shadow(&self) -> Option<AnimatableBoxShadow> {
        match self {
            Self::BoxShadow { shadow } => Some(*shadow),
            _ => None,
        }
    }

    /// Try to extract visibility.
    pub fn as_visibility(&self) -> Option<Visibility> {
        match self {
            Self::Visibility { value } => Some(*value),
            _ => None,
        }
    }
}

impl From<f64> for AnimatableValue {
    fn from(v: f64) -> Self {
        Self::F64 { value: v }
    }
}

impl From<[f32; 4]> for AnimatableValue {
    fn from(c: [f32; 4]) -> Self {
        Self::Color { rgba: c }
    }
}

impl From<AnimatableEdgeInsets> for AnimatableValue {
    fn from(e: AnimatableEdgeInsets) -> Self {
        Self::EdgeInsets { insets: e }
    }
}

impl From<AnimatableTransform> for AnimatableValue {
    fn from(t: AnimatableTransform) -> Self {
        Self::Transform { transform: t }
    }
}

impl From<AnimatableBoxShadow> for AnimatableValue {
    fn from(s: AnimatableBoxShadow) -> Self {
        Self::BoxShadow { shadow: s }
    }
}

impl From<Visibility> for AnimatableValue {
    fn from(v: Visibility) -> Self {
        Self::Visibility { value: v }
    }
}

/// Enum mapping to animatable IR node properties.
///
/// These properties correspond to fields on IR spec structs (FlexContainerSpec,
/// TextSpec, etc.) that can be animated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimatableProperty {
    // Geometry properties
    Width,
    Height,
    MinWidth,
    MinHeight,
    MaxWidth,
    MaxHeight,

    // Spacing properties (individual)
    PaddingTop,
    PaddingRight,
    PaddingBottom,
    PaddingLeft,
    MarginTop,
    MarginRight,
    MarginBottom,
    MarginLeft,

    // Spacing properties (grouped)
    Padding,
    Margin,

    // Visual properties
    Opacity,
    Visibility,
    CornerRadius,
    BorderWidth,
    BorderColor,

    // Background
    BackgroundColor,

    // Text properties
    FontSize,
    TextColor,

    // Transform properties (individual)
    TranslateX,
    TranslateY,
    ScaleX,
    ScaleY,
    Rotate,

    // Transform (grouped)
    Transform,

    // Shadow properties (individual)
    BoxShadowOffsetX,
    BoxShadowOffsetY,
    BoxShadowBlur,
    BoxShadowColor,

    // Shadow (grouped)
    BoxShadow,
}

impl AnimatableProperty {
    /// Returns the expected value type for this property.
    pub fn value_type(&self) -> AnimatableValueType {
        match self {
            // Geometry
            Self::Width
            | Self::Height
            | Self::MinWidth
            | Self::MinHeight
            | Self::MaxWidth
            | Self::MaxHeight => AnimatableValueType::F64,

            // Spacing (individual)
            Self::PaddingTop
            | Self::PaddingRight
            | Self::PaddingBottom
            | Self::PaddingLeft
            | Self::MarginTop
            | Self::MarginRight
            | Self::MarginBottom
            | Self::MarginLeft => AnimatableValueType::F64,

            // Spacing (grouped)
            Self::Padding | Self::Margin => AnimatableValueType::EdgeInsets,

            // Visual
            Self::Opacity | Self::CornerRadius | Self::BorderWidth => AnimatableValueType::F64,
            Self::Visibility => AnimatableValueType::Visibility,
            Self::BorderColor | Self::BackgroundColor => AnimatableValueType::Color,

            // Text
            Self::FontSize => AnimatableValueType::F64,
            Self::TextColor => AnimatableValueType::Color,

            // Transform (individual)
            Self::TranslateX
            | Self::TranslateY
            | Self::ScaleX
            | Self::ScaleY
            | Self::Rotate => AnimatableValueType::F64,

            // Transform (grouped)
            Self::Transform => AnimatableValueType::Transform,

            // Shadow (individual)
            Self::BoxShadowOffsetX | Self::BoxShadowOffsetY | Self::BoxShadowBlur => {
                AnimatableValueType::F64
            }
            Self::BoxShadowColor => AnimatableValueType::Color,

            // Shadow (grouped)
            Self::BoxShadow => AnimatableValueType::BoxShadow,
        }
    }

    /// Returns true if this is a layout-affecting property that requires Taffy relayout.
    pub fn affects_layout(&self) -> bool {
        matches!(
            self,
            Self::Width
                | Self::Height
                | Self::MinWidth
                | Self::MinHeight
                | Self::MaxWidth
                | Self::MaxHeight
                | Self::PaddingTop
                | Self::PaddingRight
                | Self::PaddingBottom
                | Self::PaddingLeft
                | Self::Padding
                | Self::MarginTop
                | Self::MarginRight
                | Self::MarginBottom
                | Self::MarginLeft
                | Self::Margin
                | Self::FontSize
                | Self::Visibility // Collapsed visibility affects layout
        )
    }

    /// Returns true if this is a visual-only property (no layout impact).
    pub fn is_visual_only(&self) -> bool {
        !self.affects_layout()
    }
}

/// Expected value type for an animatable property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimatableValueType {
    F64,
    Color,
    EdgeInsets,
    Transform,
    BoxShadow,
    Visibility,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_id_uniqueness() {
        let id1 = AnimationId::new();
        let id2 = AnimationId::new();
        let id3 = AnimationId::new();

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_animation_state_default() {
        assert_eq!(AnimationState::default(), AnimationState::Pending);
    }

    #[test]
    fn test_animatable_value_conversions() {
        // f64
        let v: AnimatableValue = 42.0.into();
        assert_eq!(v.as_f64(), Some(42.0));
        assert_eq!(v.as_color(), None);

        // Color
        let v: AnimatableValue = [1.0, 0.5, 0.0, 1.0].into();
        assert_eq!(v.as_color(), Some([1.0, 0.5, 0.0, 1.0]));
        assert_eq!(v.as_f64(), None);

        // EdgeInsets
        let e = AnimatableEdgeInsets::uniform(10.0);
        let v: AnimatableValue = e.into();
        let extracted = v.as_edge_insets().unwrap();
        assert_eq!(extracted.top, 10.0);
        assert_eq!(extracted.right, 10.0);

        // Transform
        let t = AnimatableTransform {
            translate_x: 100.0,
            scale_x: 2.0,
            ..Default::default()
        };
        let v: AnimatableValue = t.into();
        let extracted = v.as_transform().unwrap();
        assert_eq!(extracted.translate_x, 100.0);
        assert_eq!(extracted.scale_x, 2.0);
    }

    #[test]
    fn test_property_value_types() {
        assert_eq!(
            AnimatableProperty::Width.value_type(),
            AnimatableValueType::F64
        );
        assert_eq!(
            AnimatableProperty::BackgroundColor.value_type(),
            AnimatableValueType::Color
        );
        assert_eq!(
            AnimatableProperty::Padding.value_type(),
            AnimatableValueType::EdgeInsets
        );
        assert_eq!(
            AnimatableProperty::Transform.value_type(),
            AnimatableValueType::Transform
        );
        assert_eq!(
            AnimatableProperty::BoxShadow.value_type(),
            AnimatableValueType::BoxShadow
        );
    }

    #[test]
    fn test_property_layout_impact() {
        // Layout-affecting
        assert!(AnimatableProperty::Width.affects_layout());
        assert!(AnimatableProperty::Height.affects_layout());
        assert!(AnimatableProperty::PaddingTop.affects_layout());
        assert!(AnimatableProperty::Margin.affects_layout());

        // Visual-only
        assert!(AnimatableProperty::Opacity.is_visual_only());
        assert!(AnimatableProperty::BackgroundColor.is_visual_only());
        assert!(AnimatableProperty::Transform.is_visual_only());
        assert!(AnimatableProperty::BoxShadow.is_visual_only());
    }
}
