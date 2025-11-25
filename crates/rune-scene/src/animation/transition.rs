//! Transition system for CSS-like property transitions.
//!
//! This module provides:
//! - `TransitionSpec`: Configuration for a single property transition
//! - `TransitionTarget`: Specifies which property or properties to transition
//! - `TransitionGroup`: Collection of transition specs for multiple properties
//! - `ActiveTransition`: Runtime state for an in-progress transition
//!
//! # Example
//!
//! ```ignore
//! use rune_scene::animation::transition::{TransitionSpec, TransitionTarget};
//! use rune_scene::animation::easing::EasingFunction;
//!
//! // Transition opacity over 300ms with ease-out
//! let spec = TransitionSpec {
//!     target: TransitionTarget::Property { property: AnimatableProperty::Opacity },
//!     duration_ms: 300.0,
//!     delay_ms: 0.0,
//!     easing: EasingFunction::EaseOut,
//! };
//! ```

use serde::{Deserialize, Serialize};

use super::easing::EasingFunction;
use super::interpolate::Interpolate;
use super::types::{AnimatableProperty, AnimatableValue, AnimationId, AnimationState};

/// Specifies which property or properties a transition applies to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransitionTarget {
    /// Transition a specific property.
    Property { property: AnimatableProperty },
    /// Transition all animatable properties (CSS `transition: all`).
    All,
}

impl Default for TransitionTarget {
    fn default() -> Self {
        Self::All
    }
}

/// Specification for a single property transition.
///
/// This struct defines how a property should transition when its value changes,
/// similar to CSS `transition` declarations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransitionSpec {
    /// Which property or properties this transition applies to.
    pub target: TransitionTarget,
    /// Duration of the transition in milliseconds.
    pub duration_ms: f32,
    /// Delay before the transition starts in milliseconds.
    pub delay_ms: f32,
    /// Easing function for the transition timing.
    pub easing: EasingFunction,
}

impl Default for TransitionSpec {
    fn default() -> Self {
        Self {
            target: TransitionTarget::All,
            duration_ms: 300.0,
            delay_ms: 0.0,
            easing: EasingFunction::Ease,
        }
    }
}

impl TransitionSpec {
    /// Create a new transition spec for a specific property.
    pub fn property(property: AnimatableProperty, duration_ms: f32) -> Self {
        Self {
            target: TransitionTarget::Property { property },
            duration_ms,
            delay_ms: 0.0,
            easing: EasingFunction::Ease,
        }
    }

    /// Create a transition spec that applies to all properties.
    pub fn all(duration_ms: f32) -> Self {
        Self {
            target: TransitionTarget::All,
            duration_ms,
            delay_ms: 0.0,
            easing: EasingFunction::Ease,
        }
    }

    /// Set the delay for this transition.
    pub fn with_delay(mut self, delay_ms: f32) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    /// Set the easing function for this transition.
    pub fn with_easing(mut self, easing: EasingFunction) -> Self {
        self.easing = easing;
        self
    }

    /// Check if this spec applies to a given property.
    pub fn applies_to(&self, property: AnimatableProperty) -> bool {
        match &self.target {
            TransitionTarget::All => true,
            TransitionTarget::Property { property: p } => *p == property,
        }
    }
}

/// A group of transition specifications for multiple properties.
///
/// This allows defining different transition timings for different properties,
/// similar to CSS `transition: opacity 300ms ease, transform 500ms ease-out`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransitionGroup {
    /// Individual transition specifications.
    pub specs: Vec<TransitionSpec>,
}

impl TransitionGroup {
    /// Create a new empty transition group.
    pub fn new() -> Self {
        Self { specs: Vec::new() }
    }

    /// Create a transition group with a single "all" transition.
    pub fn all(duration_ms: f32, easing: EasingFunction) -> Self {
        Self {
            specs: vec![TransitionSpec {
                target: TransitionTarget::All,
                duration_ms,
                delay_ms: 0.0,
                easing,
            }],
        }
    }

    /// Add a transition spec to the group.
    pub fn with(mut self, spec: TransitionSpec) -> Self {
        self.specs.push(spec);
        self
    }

    /// Add a property-specific transition.
    pub fn with_property(
        mut self,
        property: AnimatableProperty,
        duration_ms: f32,
        easing: EasingFunction,
    ) -> Self {
        self.specs.push(TransitionSpec {
            target: TransitionTarget::Property { property },
            duration_ms,
            delay_ms: 0.0,
            easing,
        });
        self
    }

    /// Find the transition spec that applies to a given property.
    ///
    /// Returns the most specific match (property-specific over "all").
    pub fn spec_for(&self, property: AnimatableProperty) -> Option<&TransitionSpec> {
        // First look for a property-specific spec
        let specific = self.specs.iter().find(|s| {
            matches!(&s.target, TransitionTarget::Property { property: p } if *p == property)
        });

        if specific.is_some() {
            return specific;
        }

        // Fall back to "all" spec
        self.specs
            .iter()
            .find(|s| matches!(s.target, TransitionTarget::All))
    }

    /// Check if any transition is defined for a property.
    pub fn has_transition_for(&self, property: AnimatableProperty) -> bool {
        self.spec_for(property).is_some()
    }

    /// Returns true if the group is empty.
    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }
}

/// An active transition that is currently in progress.
///
/// This struct tracks the runtime state of a transition, including elapsed time
/// and the values being interpolated between.
#[derive(Debug, Clone)]
pub struct ActiveTransition {
    /// Unique identifier for this transition.
    pub id: AnimationId,
    /// The node ID this transition applies to.
    pub node_id: String,
    /// The property being transitioned.
    pub property: AnimatableProperty,
    /// Starting value of the transition.
    pub from_value: AnimatableValue,
    /// Target value of the transition.
    pub to_value: AnimatableValue,
    /// Total duration in milliseconds.
    pub duration_ms: f32,
    /// Delay before transition starts in milliseconds.
    pub delay_ms: f32,
    /// Time elapsed since transition was created in milliseconds.
    pub elapsed_ms: f32,
    /// Easing function for timing.
    pub easing: EasingFunction,
    /// Current state of the transition.
    pub state: AnimationState,
}

impl ActiveTransition {
    /// Create a new active transition.
    pub fn new(
        node_id: String,
        property: AnimatableProperty,
        from_value: AnimatableValue,
        to_value: AnimatableValue,
        spec: &TransitionSpec,
    ) -> Self {
        Self {
            id: AnimationId::new(),
            node_id,
            property,
            from_value,
            to_value,
            duration_ms: spec.duration_ms,
            delay_ms: spec.delay_ms,
            elapsed_ms: 0.0,
            easing: spec.easing,
            state: if spec.delay_ms > 0.0 {
                AnimationState::Pending
            } else {
                AnimationState::Running
            },
        }
    }

    /// Get the current interpolated value of the transition.
    pub fn current_value(&self) -> AnimatableValue {
        match self.state {
            AnimationState::Pending => self.from_value.clone(),
            AnimationState::Finished => self.to_value.clone(),
            AnimationState::Cancelled => self.from_value.clone(),
            AnimationState::Running | AnimationState::Paused => {
                let active_elapsed = (self.elapsed_ms - self.delay_ms).max(0.0);
                let progress = if self.duration_ms > 0.0 {
                    (active_elapsed / self.duration_ms).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let eased_progress = self.easing.evaluate(progress);
                self.from_value.interpolate(&self.to_value, eased_progress)
            }
        }
    }

    /// Update the transition by advancing time.
    ///
    /// Returns `true` if the transition is still active (running or pending),
    /// `false` if it has finished or was cancelled.
    pub fn update(&mut self, delta_ms: f32) -> bool {
        match self.state {
            AnimationState::Finished | AnimationState::Cancelled => false,
            AnimationState::Paused => true,
            AnimationState::Pending => {
                self.elapsed_ms += delta_ms;
                if self.elapsed_ms >= self.delay_ms {
                    self.state = AnimationState::Running;
                }
                true
            }
            AnimationState::Running => {
                self.elapsed_ms += delta_ms;
                let active_elapsed = self.elapsed_ms - self.delay_ms;
                if active_elapsed >= self.duration_ms {
                    self.state = AnimationState::Finished;
                    false
                } else {
                    true
                }
            }
        }
    }

    /// Pause the transition.
    pub fn pause(&mut self) {
        if self.state == AnimationState::Running || self.state == AnimationState::Pending {
            self.state = AnimationState::Paused;
        }
    }

    /// Resume a paused transition.
    pub fn resume(&mut self) {
        if self.state == AnimationState::Paused {
            if self.elapsed_ms < self.delay_ms {
                self.state = AnimationState::Pending;
            } else {
                self.state = AnimationState::Running;
            }
        }
    }

    /// Cancel the transition.
    pub fn cancel(&mut self) {
        self.state = AnimationState::Cancelled;
    }

    /// Check if this transition is still active.
    pub fn is_active(&self) -> bool {
        matches!(
            self.state,
            AnimationState::Pending | AnimationState::Running | AnimationState::Paused
        )
    }

    /// Check if this transition has completed successfully.
    pub fn is_finished(&self) -> bool {
        self.state == AnimationState::Finished
    }

    /// Get the progress of this transition (0.0 to 1.0).
    pub fn progress(&self) -> f32 {
        let active_elapsed = (self.elapsed_ms - self.delay_ms).max(0.0);
        if self.duration_ms > 0.0 {
            (active_elapsed / self.duration_ms).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    /// Retarget the transition to a new destination value.
    ///
    /// This is used when a property changes while a transition is in progress.
    /// The transition continues from its current value to the new target.
    pub fn retarget(&mut self, new_to_value: AnimatableValue, spec: &TransitionSpec) {
        // Start from current interpolated value
        self.from_value = self.current_value();
        self.to_value = new_to_value;

        // Reset timing with new spec
        self.duration_ms = spec.duration_ms;
        self.delay_ms = spec.delay_ms;
        self.elapsed_ms = 0.0;
        self.easing = spec.easing;
        self.state = if spec.delay_ms > 0.0 {
            AnimationState::Pending
        } else {
            AnimationState::Running
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::types::AnimatableProperty;

    #[test]
    fn test_transition_spec_defaults() {
        let spec = TransitionSpec::default();
        assert_eq!(spec.target, TransitionTarget::All);
        assert_eq!(spec.duration_ms, 300.0);
        assert_eq!(spec.delay_ms, 0.0);
        assert_eq!(spec.easing, EasingFunction::Ease);
    }

    #[test]
    fn test_transition_spec_builders() {
        let spec = TransitionSpec::property(AnimatableProperty::Opacity, 500.0)
            .with_delay(100.0)
            .with_easing(EasingFunction::EaseOut);

        assert_eq!(
            spec.target,
            TransitionTarget::Property {
                property: AnimatableProperty::Opacity
            }
        );
        assert_eq!(spec.duration_ms, 500.0);
        assert_eq!(spec.delay_ms, 100.0);
        assert_eq!(spec.easing, EasingFunction::EaseOut);
    }

    #[test]
    fn test_transition_spec_applies_to() {
        let all_spec = TransitionSpec::all(300.0);
        assert!(all_spec.applies_to(AnimatableProperty::Opacity));
        assert!(all_spec.applies_to(AnimatableProperty::Width));

        let opacity_spec = TransitionSpec::property(AnimatableProperty::Opacity, 300.0);
        assert!(opacity_spec.applies_to(AnimatableProperty::Opacity));
        assert!(!opacity_spec.applies_to(AnimatableProperty::Width));
    }

    #[test]
    fn test_transition_group_spec_lookup() {
        let group = TransitionGroup::new()
            .with(TransitionSpec::all(300.0).with_easing(EasingFunction::Ease))
            .with_property(AnimatableProperty::Opacity, 500.0, EasingFunction::EaseOut);

        // Opacity should use the specific spec
        let opacity_spec = group.spec_for(AnimatableProperty::Opacity).unwrap();
        assert_eq!(opacity_spec.duration_ms, 500.0);
        assert_eq!(opacity_spec.easing, EasingFunction::EaseOut);

        // Width should fall back to "all" spec
        let width_spec = group.spec_for(AnimatableProperty::Width).unwrap();
        assert_eq!(width_spec.duration_ms, 300.0);
        assert_eq!(width_spec.easing, EasingFunction::Ease);
    }

    #[test]
    fn test_active_transition_lifecycle() {
        let spec = TransitionSpec::all(100.0);
        let mut transition = ActiveTransition::new(
            "node1".to_string(),
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 1.0 },
            &spec,
        );

        // Should start running (no delay)
        assert_eq!(transition.state, AnimationState::Running);
        assert!(transition.is_active());

        // Update partially
        assert!(transition.update(50.0));
        assert_eq!(transition.state, AnimationState::Running);
        assert!((transition.progress() - 0.5).abs() < 0.01);

        // Update to completion
        assert!(!transition.update(60.0));
        assert_eq!(transition.state, AnimationState::Finished);
        assert!(transition.is_finished());
        assert!(!transition.is_active());
    }

    #[test]
    fn test_active_transition_with_delay() {
        let spec = TransitionSpec::all(100.0).with_delay(50.0);
        let mut transition = ActiveTransition::new(
            "node1".to_string(),
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 1.0 },
            &spec,
        );

        // Should start pending
        assert_eq!(transition.state, AnimationState::Pending);

        // During delay, value should be from_value
        transition.update(25.0);
        assert_eq!(transition.state, AnimationState::Pending);
        assert_eq!(transition.current_value().as_f64(), Some(0.0));

        // After delay, should be running
        transition.update(30.0);
        assert_eq!(transition.state, AnimationState::Running);
    }

    #[test]
    fn test_active_transition_pause_resume() {
        let spec = TransitionSpec::all(100.0);
        let mut transition = ActiveTransition::new(
            "node1".to_string(),
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 1.0 },
            &spec,
        );

        transition.update(50.0);
        let value_before_pause = transition.current_value();

        transition.pause();
        assert_eq!(transition.state, AnimationState::Paused);

        // Time passes but value doesn't change
        transition.update(100.0);
        assert_eq!(transition.state, AnimationState::Paused);
        assert_eq!(transition.current_value(), value_before_pause);

        // Resume and continue
        transition.resume();
        assert_eq!(transition.state, AnimationState::Running);
    }

    #[test]
    fn test_active_transition_current_value() {
        let spec = TransitionSpec::all(100.0).with_easing(EasingFunction::Linear);
        let mut transition = ActiveTransition::new(
            "node1".to_string(),
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 100.0 },
            &spec,
        );

        // At start
        let v = transition.current_value().as_f64().unwrap();
        assert!((v - 0.0).abs() < 0.01);

        // At 50%
        transition.update(50.0);
        let v = transition.current_value().as_f64().unwrap();
        assert!((v - 50.0).abs() < 0.01);

        // At 100%
        transition.update(50.0);
        let v = transition.current_value().as_f64().unwrap();
        assert!((v - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_active_transition_retarget() {
        let spec = TransitionSpec::all(100.0).with_easing(EasingFunction::Linear);
        let mut transition = ActiveTransition::new(
            "node1".to_string(),
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 100.0 },
            &spec,
        );

        // Progress to 50%
        transition.update(50.0);
        let mid_value = transition.current_value().as_f64().unwrap();
        assert!((mid_value - 50.0).abs() < 0.01);

        // Retarget to new value
        let new_spec = TransitionSpec::all(200.0).with_easing(EasingFunction::Linear);
        transition.retarget(AnimatableValue::F64 { value: 0.0 }, &new_spec);

        // Should start from current value (50) going to new target (0)
        assert_eq!(transition.state, AnimationState::Running);
        assert_eq!(transition.elapsed_ms, 0.0);
        assert_eq!(transition.duration_ms, 200.0);

        let start_value = transition.current_value().as_f64().unwrap();
        assert!((start_value - 50.0).abs() < 0.01);

        // Progress to end
        transition.update(200.0);
        let end_value = transition.current_value().as_f64().unwrap();
        assert!((end_value - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_zero_duration_transition() {
        let spec = TransitionSpec::all(0.0);
        let mut transition = ActiveTransition::new(
            "node1".to_string(),
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 1.0 },
            &spec,
        );

        // Should immediately be at end value
        let v = transition.current_value().as_f64().unwrap();
        assert!((v - 1.0).abs() < 0.01);

        // First update should complete it
        assert!(!transition.update(1.0));
        assert!(transition.is_finished());
    }
}
