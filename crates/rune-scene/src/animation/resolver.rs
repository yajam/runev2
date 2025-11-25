//! Animated property resolver for integrating animations into the rendering pipeline.
//!
//! The `AnimatedPropertyResolver` queries the `AnimationManager` for current animated
//! values during rendering, allowing elements to smoothly transition between states.

use super::manager::AnimationManager;
use super::transform::{Transform2D, TransformOrigin};
use super::types::{
    AnimatableBoxShadow, AnimatableEdgeInsets, AnimatableProperty, AnimatableTransform,
    AnimatableValue, Visibility,
};

/// Resolves animated property values for rendering.
///
/// This struct provides a convenient interface for querying animated values
/// during the rendering pass. It wraps an `AnimationManager` reference and
/// provides type-specific resolution methods.
///
/// # Usage
///
/// ```ignore
/// let resolver = AnimatedPropertyResolver::new(&animation_manager);
///
/// // Resolve an f64 property with fallback to base value
/// let width = resolver.resolve_f64("node_1", AnimatableProperty::Width, 100.0);
///
/// // Resolve a color property
/// let bg_color = resolver.resolve_color("node_1", AnimatableProperty::BackgroundColor, [1.0, 1.0, 1.0, 1.0]);
/// ```
pub struct AnimatedPropertyResolver<'a> {
    animation_manager: &'a AnimationManager,
}

impl<'a> AnimatedPropertyResolver<'a> {
    /// Create a new resolver wrapping the given animation manager.
    pub fn new(animation_manager: &'a AnimationManager) -> Self {
        Self { animation_manager }
    }

    /// Get the underlying animation manager reference.
    pub fn animation_manager(&self) -> &AnimationManager {
        self.animation_manager
    }

    /// Resolve an f64 property, returning the animated value if active, otherwise the base value.
    ///
    /// # Arguments
    /// * `node_id` - The ID of the node to query
    /// * `property` - The property to resolve
    /// * `base` - The base value to use if no animation is active
    ///
    /// # Returns
    /// The current animated value if an animation is active, otherwise `base`.
    pub fn resolve_f64(&self, node_id: &str, property: AnimatableProperty, base: f64) -> f64 {
        if let Some(value) = self.animation_manager.get_animated_value(node_id, property) {
            value.as_f64().unwrap_or(base)
        } else {
            base
        }
    }

    /// Resolve a color property, returning the animated value if active, otherwise the base value.
    ///
    /// # Arguments
    /// * `node_id` - The ID of the node to query
    /// * `property` - The property to resolve
    /// * `base` - The base RGBA color to use if no animation is active
    ///
    /// # Returns
    /// The current animated color if an animation is active, otherwise `base`.
    pub fn resolve_color(
        &self,
        node_id: &str,
        property: AnimatableProperty,
        base: [f32; 4],
    ) -> [f32; 4] {
        if let Some(value) = self.animation_manager.get_animated_value(node_id, property) {
            value.as_color().unwrap_or(base)
        } else {
            base
        }
    }

    /// Resolve an edge insets property (padding/margin).
    ///
    /// # Arguments
    /// * `node_id` - The ID of the node to query
    /// * `property` - The property to resolve (Padding or Margin)
    /// * `base` - The base edge insets to use if no animation is active
    ///
    /// # Returns
    /// The current animated edge insets if an animation is active, otherwise `base`.
    pub fn resolve_edge_insets(
        &self,
        node_id: &str,
        property: AnimatableProperty,
        base: AnimatableEdgeInsets,
    ) -> AnimatableEdgeInsets {
        if let Some(value) = self.animation_manager.get_animated_value(node_id, property) {
            value.as_edge_insets().unwrap_or(base)
        } else {
            base
        }
    }

    /// Resolve a transform property.
    ///
    /// # Arguments
    /// * `node_id` - The ID of the node to query
    /// * `property` - The property to resolve (Transform)
    /// * `base` - The base transform to use if no animation is active
    ///
    /// # Returns
    /// The current animated transform if an animation is active, otherwise `base`.
    pub fn resolve_transform(
        &self,
        node_id: &str,
        property: AnimatableProperty,
        base: AnimatableTransform,
    ) -> AnimatableTransform {
        if let Some(value) = self.animation_manager.get_animated_value(node_id, property) {
            value.as_transform().unwrap_or(base)
        } else {
            base
        }
    }

    /// Resolve a box shadow property.
    ///
    /// # Arguments
    /// * `node_id` - The ID of the node to query
    /// * `property` - The property to resolve (BoxShadow)
    /// * `base` - The base box shadow to use if no animation is active
    ///
    /// # Returns
    /// The current animated box shadow if an animation is active, otherwise `base`.
    pub fn resolve_box_shadow(
        &self,
        node_id: &str,
        property: AnimatableProperty,
        base: AnimatableBoxShadow,
    ) -> AnimatableBoxShadow {
        if let Some(value) = self.animation_manager.get_animated_value(node_id, property) {
            value.as_box_shadow().unwrap_or(base)
        } else {
            base
        }
    }

    /// Resolve any animatable value, returning the animated value if active.
    ///
    /// # Arguments
    /// * `node_id` - The ID of the node to query
    /// * `property` - The property to resolve
    ///
    /// # Returns
    /// The current animated value if an animation is active, otherwise `None`.
    pub fn resolve(&self, node_id: &str, property: AnimatableProperty) -> Option<AnimatableValue> {
        self.animation_manager.get_animated_value(node_id, property)
    }

    /// Check if a specific node has any active animations.
    pub fn has_animations_for_node(&self, node_id: &str) -> bool {
        !self
            .animation_manager
            .get_all_animated_values(node_id)
            .is_empty()
    }

    /// Check if any animations are currently active.
    pub fn has_active_animations(&self) -> bool {
        self.animation_manager.has_active_animations()
    }

    /// Check if any layout-affecting properties are being animated for a node.
    ///
    /// This is useful for determining whether a Taffy relayout is needed.
    pub fn has_layout_animations_for_node(&self, node_id: &str) -> bool {
        let animated = self.animation_manager.get_all_animated_values(node_id);
        animated.keys().any(|prop| prop.affects_layout())
    }

    /// Check if any layout-affecting properties are being animated globally.
    ///
    /// This is useful for determining whether a Taffy relayout is needed.
    pub fn has_any_layout_animations(&self) -> bool {
        self.animation_manager.has_layout_animations()
    }

    /// Resolve a full transform matrix from animated properties.
    ///
    /// This method checks both grouped Transform animation and individual
    /// transform component animations (TranslateX, TranslateY, ScaleX, etc.),
    /// with individual components taking priority.
    ///
    /// # Arguments
    /// * `node_id` - The ID of the node to query
    /// * `base` - The base transform to use if no animation is active
    /// * `width` - Element width for transform origin calculation
    /// * `height` - Element height for transform origin calculation
    /// * `origin` - The transform origin point
    ///
    /// # Returns
    /// A Transform2D matrix with all animated components applied.
    pub fn resolve_transform_matrix(
        &self,
        node_id: &str,
        base: AnimatableTransform,
        width: f64,
        height: f64,
        origin: TransformOrigin,
    ) -> Transform2D {
        // Start with base or grouped transform animation
        let mut transform = if let Some(value) = self
            .animation_manager
            .get_animated_value(node_id, AnimatableProperty::Transform)
        {
            value.as_transform().unwrap_or(base)
        } else {
            base
        };

        // Override with individual component animations
        if let Some(value) = self
            .animation_manager
            .get_animated_value(node_id, AnimatableProperty::TranslateX)
        {
            if let Some(v) = value.as_f64() {
                transform.translate_x = v;
            }
        }

        if let Some(value) = self
            .animation_manager
            .get_animated_value(node_id, AnimatableProperty::TranslateY)
        {
            if let Some(v) = value.as_f64() {
                transform.translate_y = v;
            }
        }

        if let Some(value) = self
            .animation_manager
            .get_animated_value(node_id, AnimatableProperty::ScaleX)
        {
            if let Some(v) = value.as_f64() {
                transform.scale_x = v;
            }
        }

        if let Some(value) = self
            .animation_manager
            .get_animated_value(node_id, AnimatableProperty::ScaleY)
        {
            if let Some(v) = value.as_f64() {
                transform.scale_y = v;
            }
        }

        if let Some(value) = self
            .animation_manager
            .get_animated_value(node_id, AnimatableProperty::Rotate)
        {
            if let Some(v) = value.as_f64() {
                transform.rotate = v;
            }
        }

        // Convert to Transform2D and apply origin
        let matrix = Transform2D::from_animatable(&transform);
        matrix.with_origin(origin, width, height)
    }

    /// Resolve opacity, handling both animated and base values.
    ///
    /// # Arguments
    /// * `node_id` - The ID of the node to query
    /// * `base` - The base opacity to use if no animation is active (default 1.0)
    ///
    /// # Returns
    /// The current opacity clamped to [0.0, 1.0].
    pub fn resolve_opacity(&self, node_id: &str, base: f64) -> f64 {
        let value = self.resolve_f64(node_id, AnimatableProperty::Opacity, base);
        value.clamp(0.0, 1.0)
    }

    /// Resolve visibility state, handling both animated and base values.
    ///
    /// Note: Visibility is typically not smoothly animated but can transition
    /// discretely. The animation system will handle this as a stepped transition.
    ///
    /// # Arguments
    /// * `node_id` - The ID of the node to query
    /// * `base` - The base visibility to use if no animation is active
    ///
    /// # Returns
    /// The current visibility state.
    pub fn resolve_visibility(&self, node_id: &str, base: Visibility) -> Visibility {
        if let Some(value) = self.animation_manager.get_animated_value(node_id, AnimatableProperty::Visibility) {
            value.as_visibility().unwrap_or(base)
        } else {
            base
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::easing::EasingFunction;
    use crate::animation::transition::TransitionSpec;

    #[test]
    fn test_resolver_resolve_f64() {
        let mut manager = AnimationManager::new();

        // Start a width transition
        let spec = TransitionSpec::all(300.0).with_easing(EasingFunction::Linear);
        manager.start_transition(
            "node_1",
            AnimatableProperty::Width,
            AnimatableValue::F64 { value: 100.0 },
            AnimatableValue::F64 { value: 200.0 },
            &spec,
        );

        // Advance to midpoint
        manager.update(150.0);

        let resolver = AnimatedPropertyResolver::new(&manager);

        // Should return animated value (approximately 150)
        let width = resolver.resolve_f64("node_1", AnimatableProperty::Width, 100.0);
        assert!((width - 150.0).abs() < 1.0);

        // Non-animated property should return base
        let height = resolver.resolve_f64("node_1", AnimatableProperty::Height, 50.0);
        assert_eq!(height, 50.0);

        // Non-animated node should return base
        let other_width = resolver.resolve_f64("node_2", AnimatableProperty::Width, 300.0);
        assert_eq!(other_width, 300.0);
    }

    #[test]
    fn test_resolver_resolve_color() {
        let mut manager = AnimationManager::new();

        let spec = TransitionSpec::all(100.0).with_easing(EasingFunction::Linear);
        manager.start_transition(
            "node_1",
            AnimatableProperty::BackgroundColor,
            AnimatableValue::Color {
                rgba: [1.0, 0.0, 0.0, 1.0],
            },
            AnimatableValue::Color {
                rgba: [0.0, 0.0, 1.0, 1.0],
            },
            &spec,
        );

        // Advance to midpoint
        manager.update(50.0);

        let resolver = AnimatedPropertyResolver::new(&manager);

        let color =
            resolver.resolve_color("node_1", AnimatableProperty::BackgroundColor, [1.0; 4]);
        // Should be approximately [0.5, 0.0, 0.5, 1.0]
        assert!((color[0] - 0.5).abs() < 0.1);
        assert!((color[2] - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_resolver_has_animations() {
        let mut manager = AnimationManager::new();
        let resolver = AnimatedPropertyResolver::new(&manager);

        assert!(!resolver.has_active_animations());
        assert!(!resolver.has_animations_for_node("node_1"));

        let spec = TransitionSpec::all(100.0);
        manager.start_transition(
            "node_1",
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 1.0 },
            AnimatableValue::F64 { value: 0.0 },
            &spec,
        );

        let resolver = AnimatedPropertyResolver::new(&manager);
        assert!(resolver.has_active_animations());
        assert!(resolver.has_animations_for_node("node_1"));
        assert!(!resolver.has_animations_for_node("node_2"));
    }

    #[test]
    fn test_resolver_layout_animations() {
        let mut manager = AnimationManager::new();

        // Visual-only animation
        let spec = TransitionSpec::all(100.0);
        manager.start_transition(
            "node_1",
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 1.0 },
            AnimatableValue::F64 { value: 0.0 },
            &spec,
        );

        let resolver = AnimatedPropertyResolver::new(&manager);
        assert!(!resolver.has_layout_animations_for_node("node_1"));
        assert!(!resolver.has_any_layout_animations());

        // Layout-affecting animation
        manager.start_transition(
            "node_2",
            AnimatableProperty::Width,
            AnimatableValue::F64 { value: 100.0 },
            AnimatableValue::F64 { value: 200.0 },
            &spec,
        );

        let resolver = AnimatedPropertyResolver::new(&manager);
        assert!(resolver.has_layout_animations_for_node("node_2"));
        assert!(resolver.has_any_layout_animations());
    }
}
