//! IR schema integration types for animations.
//!
//! This module provides types that integrate animations with the IR schema:
//! - `AnimationRef`: Reference to a named animation with optional overrides
//!
//! These types enable defining animations in ViewDocuments and referencing them
//! from node specs.

use serde::{Deserialize, Serialize};

use super::keyframes::{AnimationDirection, AnimationFillMode, IterationCount};
use super::transition::TransitionGroup;

/// Reference to a named keyframe animation with optional overrides.
///
/// This struct allows IR node specs to reference animations defined in the
/// ViewDocument's animation registry, optionally overriding timing parameters.
///
/// # Example JSON
///
/// ```json
/// {
///   "name": "fade-in",
///   "duration_ms": 500,
///   "delay_ms": 100,
///   "iteration_count": { "type": "count", "count": 2 },
///   "direction": "alternate",
///   "fill_mode": "forwards"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationRef {
    /// Name of the registered animation to use.
    pub name: String,

    /// Override duration in milliseconds. If None, uses animation's default.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<f32>,

    /// Override delay in milliseconds. If None, uses animation's default.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay_ms: Option<f32>,

    /// Override iteration count. If None, uses animation's default.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iteration_count: Option<IterationCount>,

    /// Override direction. If None, uses animation's default.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<AnimationDirection>,

    /// Override fill mode. If None, uses animation's default.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_mode: Option<AnimationFillMode>,

    /// Whether the animation should start immediately when the node is rendered.
    /// Defaults to true.
    #[serde(default = "default_autoplay")]
    pub autoplay: bool,
}

fn default_autoplay() -> bool {
    true
}

impl AnimationRef {
    /// Create a new animation reference by name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            duration_ms: None,
            delay_ms: None,
            iteration_count: None,
            direction: None,
            fill_mode: None,
            autoplay: true,
        }
    }

    /// Override the duration.
    pub fn with_duration(mut self, duration_ms: f32) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    /// Override the delay.
    pub fn with_delay(mut self, delay_ms: f32) -> Self {
        self.delay_ms = Some(delay_ms);
        self
    }

    /// Override the iteration count.
    pub fn with_iterations(mut self, count: IterationCount) -> Self {
        self.iteration_count = Some(count);
        self
    }

    /// Override the direction.
    pub fn with_direction(mut self, direction: AnimationDirection) -> Self {
        self.direction = Some(direction);
        self
    }

    /// Override the fill mode.
    pub fn with_fill_mode(mut self, fill_mode: AnimationFillMode) -> Self {
        self.fill_mode = Some(fill_mode);
        self
    }

    /// Set whether the animation should autoplay.
    pub fn with_autoplay(mut self, autoplay: bool) -> Self {
        self.autoplay = autoplay;
        self
    }
}

/// Animation properties that can be attached to an IR node spec.
///
/// This struct combines both transition specs and animation references,
/// allowing nodes to have smooth property transitions and/or keyframe animations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeAnimationSpec {
    /// Transition configuration for property changes.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<TransitionGroup>,

    /// Keyframe animation to apply to this node.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animation: Option<AnimationRef>,
}

impl NodeAnimationSpec {
    /// Create an empty animation spec.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the transition group.
    pub fn with_transition(mut self, transition: TransitionGroup) -> Self {
        self.transition = Some(transition);
        self
    }

    /// Set the animation reference.
    pub fn with_animation(mut self, animation: AnimationRef) -> Self {
        self.animation = Some(animation);
        self
    }

    /// Check if this spec has any animation configuration.
    pub fn is_empty(&self) -> bool {
        self.transition.is_none() && self.animation.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_ref_builder() {
        let anim_ref = AnimationRef::new("fade-in")
            .with_duration(500.0)
            .with_delay(100.0)
            .with_iterations(IterationCount::Count { count: 2.0 })
            .with_direction(AnimationDirection::Alternate)
            .with_fill_mode(AnimationFillMode::Forwards);

        assert_eq!(anim_ref.name, "fade-in");
        assert_eq!(anim_ref.duration_ms, Some(500.0));
        assert_eq!(anim_ref.delay_ms, Some(100.0));
        assert_eq!(
            anim_ref.iteration_count,
            Some(IterationCount::Count { count: 2.0 })
        );
        assert_eq!(anim_ref.direction, Some(AnimationDirection::Alternate));
        assert_eq!(anim_ref.fill_mode, Some(AnimationFillMode::Forwards));
        assert!(anim_ref.autoplay);
    }

    #[test]
    fn test_animation_ref_serialization() {
        let anim_ref = AnimationRef::new("slide-in").with_duration(300.0);

        let json = serde_json::to_string(&anim_ref).unwrap();
        assert!(json.contains("\"name\":\"slide-in\""));
        assert!(json.contains("\"duration_ms\":300.0"));

        let parsed: AnimationRef = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "slide-in");
        assert_eq!(parsed.duration_ms, Some(300.0));
    }

    #[test]
    fn test_node_animation_spec_is_empty() {
        let empty = NodeAnimationSpec::new();
        assert!(empty.is_empty());

        let with_anim = NodeAnimationSpec::new().with_animation(AnimationRef::new("test"));
        assert!(!with_anim.is_empty());
    }
}
