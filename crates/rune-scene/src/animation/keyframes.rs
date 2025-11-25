//! Keyframe animation system for CSS-like multi-step animations.
//!
//! This module provides:
//! - `Keyframe`: A single point in an animation with property values
//! - `KeyframeAnimation`: Definition of a complete keyframe animation
//! - `ActiveKeyframeAnimation`: Runtime state for an in-progress animation
//!
//! # CSS Parity
//!
//! This implementation aims for parity with CSS `@keyframes` and `animation` properties:
//! - `animation-duration`, `animation-delay`
//! - `animation-iteration-count` (including `infinite`)
//! - `animation-direction` (`normal`, `reverse`, `alternate`, `alternate-reverse`)
//! - `animation-fill-mode` (`none`, `forwards`, `backwards`, `both`)
//! - `animation-play-state` (`running`, `paused`)
//! - Per-keyframe easing functions
//!
//! # Example
//!
//! ```ignore
//! use rune_scene::animation::keyframes::*;
//! use rune_scene::animation::{AnimatableProperty, AnimatableValue, EasingFunction};
//!
//! // Define a fade-in animation
//! let animation = KeyframeAnimation::new("fade-in")
//!     .duration_ms(500.0)
//!     .keyframe(0.0, |kf| {
//!         kf.set(AnimatableProperty::Opacity, AnimatableValue::F64 { value: 0.0 })
//!     })
//!     .keyframe(1.0, |kf| {
//!         kf.set(AnimatableProperty::Opacity, AnimatableValue::F64 { value: 1.0 })
//!     });
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::easing::EasingFunction;
use super::interpolate::Interpolate;
use super::types::{AnimatableProperty, AnimatableValue, AnimationId, AnimationState};

/// How many times an animation should repeat.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IterationCount {
    /// Run the animation a specific number of times (can be fractional).
    Count { count: f32 },
    /// Run the animation indefinitely.
    Infinite,
}

impl Default for IterationCount {
    fn default() -> Self {
        Self::Count { count: 1.0 }
    }
}

impl IterationCount {
    /// Check if the animation should continue given the current iteration.
    pub fn should_continue(&self, current_iteration: f32) -> bool {
        match self {
            Self::Infinite => true,
            Self::Count { count } => current_iteration < *count,
        }
    }
}

/// Direction of animation playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationDirection {
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

impl AnimationDirection {
    /// Determine if a specific iteration should play in reverse.
    pub fn is_reversed(&self, iteration: u32) -> bool {
        match self {
            Self::Normal => false,
            Self::Reverse => true,
            Self::Alternate => iteration % 2 == 1,
            Self::AlternateReverse => iteration % 2 == 0,
        }
    }
}

/// What values to apply before/after the animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationFillMode {
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

impl AnimationFillMode {
    /// Should apply values before animation starts (during delay)?
    pub fn applies_backwards(&self) -> bool {
        matches!(self, Self::Backwards | Self::Both)
    }

    /// Should retain values after animation ends?
    pub fn applies_forwards(&self) -> bool {
        matches!(self, Self::Forwards | Self::Both)
    }
}

/// Current play state of an animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationPlayState {
    /// Animation is running.
    #[default]
    Running,
    /// Animation is paused.
    Paused,
}

/// A single keyframe in an animation sequence.
///
/// Each keyframe specifies property values at a specific point in the animation
/// timeline, identified by an offset from 0.0 (start) to 1.0 (end).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Keyframe {
    /// Position in the animation timeline (0.0 to 1.0).
    pub offset: f32,
    /// Property values at this keyframe.
    pub values: HashMap<AnimatableProperty, AnimatableValue>,
    /// Easing function to use when interpolating TO this keyframe.
    /// If None, uses the animation's default or linear.
    pub easing: Option<EasingFunction>,
}

impl Keyframe {
    /// Create a new keyframe at the given offset.
    pub fn new(offset: f32) -> Self {
        Self {
            offset: offset.clamp(0.0, 1.0),
            values: HashMap::new(),
            easing: None,
        }
    }

    /// Create a keyframe at the start (0%).
    pub fn start() -> Self {
        Self::new(0.0)
    }

    /// Create a keyframe at the end (100%).
    pub fn end() -> Self {
        Self::new(1.0)
    }

    /// Set a property value for this keyframe.
    pub fn set(mut self, property: AnimatableProperty, value: AnimatableValue) -> Self {
        self.values.insert(property, value);
        self
    }

    /// Set the easing function for interpolating to this keyframe.
    pub fn with_easing(mut self, easing: EasingFunction) -> Self {
        self.easing = Some(easing);
        self
    }

    /// Get a property value from this keyframe.
    pub fn get(&self, property: AnimatableProperty) -> Option<&AnimatableValue> {
        self.values.get(&property)
    }
}

/// Definition of a keyframe animation.
///
/// This struct defines the complete specification of an animation,
/// including all keyframes and timing parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyframeAnimation {
    /// Name of the animation (for referencing in registries).
    pub name: String,
    /// Ordered list of keyframes (sorted by offset).
    pub keyframes: Vec<Keyframe>,
    /// Total duration of one iteration in milliseconds.
    pub duration_ms: f32,
    /// Delay before the animation starts in milliseconds.
    pub delay_ms: f32,
    /// Number of times to repeat the animation.
    pub iteration_count: IterationCount,
    /// Direction of playback.
    pub direction: AnimationDirection,
    /// Fill mode (what values to apply before/after).
    pub fill_mode: AnimationFillMode,
    /// Default easing function for keyframes without explicit easing.
    pub default_easing: EasingFunction,
}

impl KeyframeAnimation {
    /// Create a new keyframe animation with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            keyframes: Vec::new(),
            duration_ms: 0.0,
            delay_ms: 0.0,
            iteration_count: IterationCount::Count { count: 1.0 },
            direction: AnimationDirection::Normal,
            fill_mode: AnimationFillMode::None,
            default_easing: EasingFunction::Ease,
        }
    }

    /// Set the animation duration.
    pub fn duration_ms(mut self, duration: f32) -> Self {
        self.duration_ms = duration;
        self
    }

    /// Set the animation delay.
    pub fn delay_ms(mut self, delay: f32) -> Self {
        self.delay_ms = delay;
        self
    }

    /// Set the iteration count.
    pub fn iterations(mut self, count: IterationCount) -> Self {
        self.iteration_count = count;
        self
    }

    /// Set the animation direction.
    pub fn direction(mut self, direction: AnimationDirection) -> Self {
        self.direction = direction;
        self
    }

    /// Set the fill mode.
    pub fn fill_mode(mut self, fill_mode: AnimationFillMode) -> Self {
        self.fill_mode = fill_mode;
        self
    }

    /// Set the default easing function.
    pub fn default_easing(mut self, easing: EasingFunction) -> Self {
        self.default_easing = easing;
        self
    }

    /// Add a keyframe using a builder function.
    pub fn keyframe<F>(mut self, offset: f32, builder: F) -> Self
    where
        F: FnOnce(Keyframe) -> Keyframe,
    {
        let kf = builder(Keyframe::new(offset));
        self.keyframes.push(kf);
        // Keep keyframes sorted by offset
        self.keyframes.sort_by(|a, b| {
            a.offset
                .partial_cmp(&b.offset)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self
    }

    /// Add a pre-built keyframe.
    pub fn add_keyframe(mut self, keyframe: Keyframe) -> Self {
        self.keyframes.push(keyframe);
        self.keyframes.sort_by(|a, b| {
            a.offset
                .partial_cmp(&b.offset)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self
    }

    /// Get all properties that are animated by this animation.
    pub fn animated_properties(&self) -> Vec<AnimatableProperty> {
        let mut props: Vec<AnimatableProperty> = self
            .keyframes
            .iter()
            .flat_map(|kf| kf.values.keys().copied())
            .collect();
        props.sort_by_key(|p| format!("{:?}", p));
        props.dedup();
        props
    }

    /// Find the keyframes surrounding a given offset.
    ///
    /// Returns (from_keyframe, to_keyframe, local_progress) where local_progress
    /// is 0.0-1.0 between the two keyframes.
    pub fn find_keyframes(&self, offset: f32) -> Option<(&Keyframe, &Keyframe, f32)> {
        if self.keyframes.is_empty() {
            return None;
        }

        let offset = offset.clamp(0.0, 1.0);

        // Find the keyframes that bracket this offset
        let mut from_idx = 0;
        let mut to_idx = 0;

        for (i, kf) in self.keyframes.iter().enumerate() {
            if kf.offset <= offset {
                from_idx = i;
            }
            if kf.offset >= offset {
                to_idx = i;
                break;
            }
            to_idx = i;
        }

        let from_kf = &self.keyframes[from_idx];
        let to_kf = &self.keyframes[to_idx];

        // Calculate local progress between the two keyframes
        let local_progress = if from_idx == to_idx {
            0.0
        } else {
            let range = to_kf.offset - from_kf.offset;
            if range > 0.0 {
                (offset - from_kf.offset) / range
            } else {
                0.0
            }
        };

        Some((from_kf, to_kf, local_progress))
    }

    /// Get the value of a property at a given offset.
    pub fn value_at(&self, property: AnimatableProperty, offset: f32) -> Option<AnimatableValue> {
        let (from_kf, to_kf, local_progress) = self.find_keyframes(offset)?;

        let from_value = from_kf.get(property)?;
        let to_value = to_kf.get(property).unwrap_or(from_value);

        // Get easing function (to_keyframe's easing, or default)
        let easing = to_kf.easing.unwrap_or(self.default_easing);
        let eased_progress = easing.evaluate(local_progress);

        Some(from_value.interpolate(to_value, eased_progress))
    }
}

/// An active keyframe animation that is currently in progress.
#[derive(Debug, Clone)]
pub struct ActiveKeyframeAnimation {
    /// Unique identifier for this animation instance.
    pub id: AnimationId,
    /// The node this animation is applied to.
    pub node_id: String,
    /// The animation definition.
    pub animation: KeyframeAnimation,
    /// Total elapsed time since creation in milliseconds.
    pub elapsed_ms: f32,
    /// Current iteration (0-indexed, can be fractional).
    pub current_iteration: f32,
    /// Current state of the animation.
    pub state: AnimationState,
    /// Play state (running/paused).
    pub play_state: AnimationPlayState,
}

impl ActiveKeyframeAnimation {
    /// Create a new active keyframe animation.
    pub fn new(node_id: String, animation: KeyframeAnimation) -> Self {
        let initial_state = if animation.delay_ms > 0.0 {
            AnimationState::Pending
        } else {
            AnimationState::Running
        };

        Self {
            id: AnimationId::new(),
            node_id,
            animation,
            elapsed_ms: 0.0,
            current_iteration: 0.0,
            state: initial_state,
            play_state: AnimationPlayState::Running,
        }
    }

    /// Get the current offset within the animation (0.0 to 1.0).
    ///
    /// Accounts for direction and current iteration.
    pub fn current_offset(&self) -> f32 {
        if self.animation.duration_ms <= 0.0 {
            return 1.0;
        }

        let active_elapsed = (self.elapsed_ms - self.animation.delay_ms).max(0.0);
        let iteration_progress = active_elapsed / self.animation.duration_ms;

        // Get the fractional part (position within current iteration)
        let raw_offset = iteration_progress.fract();

        // Handle completed iterations
        let offset = if iteration_progress >= 1.0 && raw_offset == 0.0 {
            1.0
        } else {
            raw_offset
        };

        // Apply direction
        let iteration = self.current_iteration.floor() as u32;
        if self.animation.direction.is_reversed(iteration) {
            1.0 - offset
        } else {
            offset
        }
    }

    /// Get the current value of a property.
    pub fn current_value(&self, property: AnimatableProperty) -> Option<AnimatableValue> {
        match self.state {
            AnimationState::Pending => {
                // During delay, apply backwards fill if configured
                if self.animation.fill_mode.applies_backwards() {
                    self.animation.value_at(property, 0.0)
                } else {
                    None
                }
            }
            AnimationState::Finished | AnimationState::Cancelled => {
                // After completion, apply forwards fill if configured
                if self.animation.fill_mode.applies_forwards() {
                    let final_offset = if self.animation.direction.is_reversed(
                        self.animation
                            .iteration_count
                            .should_continue(0.0)
                            .then_some(0)
                            .unwrap_or(0),
                    ) {
                        0.0
                    } else {
                        1.0
                    };
                    self.animation.value_at(property, final_offset)
                } else {
                    None
                }
            }
            AnimationState::Running | AnimationState::Paused => {
                let offset = self.current_offset();
                self.animation.value_at(property, offset)
            }
        }
    }

    /// Get all current property values.
    pub fn current_values(&self) -> HashMap<AnimatableProperty, AnimatableValue> {
        let mut values = HashMap::new();
        for property in self.animation.animated_properties() {
            if let Some(value) = self.current_value(property) {
                values.insert(property, value);
            }
        }
        values
    }

    /// Update the animation by advancing time.
    ///
    /// Returns `true` if the animation is still active.
    pub fn update(&mut self, delta_ms: f32) -> bool {
        // Don't update if paused or finished
        if self.play_state == AnimationPlayState::Paused {
            return true;
        }

        match self.state {
            AnimationState::Finished | AnimationState::Cancelled => return false,
            AnimationState::Paused => return true,
            _ => {}
        }

        self.elapsed_ms += delta_ms;

        // Check if we're past the delay
        if self.elapsed_ms < self.animation.delay_ms {
            return true;
        }

        // Transition from pending to running
        if self.state == AnimationState::Pending {
            self.state = AnimationState::Running;
        }

        // Calculate current iteration
        let active_elapsed = self.elapsed_ms - self.animation.delay_ms;
        if self.animation.duration_ms > 0.0 {
            self.current_iteration = active_elapsed / self.animation.duration_ms;
        } else {
            self.current_iteration = 1.0;
        }

        // Check if animation should end
        if !self.animation.iteration_count.should_continue(self.current_iteration) {
            self.state = AnimationState::Finished;
            return false;
        }

        true
    }

    /// Pause the animation.
    pub fn pause(&mut self) {
        self.play_state = AnimationPlayState::Paused;
    }

    /// Resume the animation.
    pub fn resume(&mut self) {
        self.play_state = AnimationPlayState::Running;
    }

    /// Cancel the animation.
    pub fn cancel(&mut self) {
        self.state = AnimationState::Cancelled;
    }

    /// Check if this animation is still active.
    pub fn is_active(&self) -> bool {
        matches!(
            self.state,
            AnimationState::Pending | AnimationState::Running | AnimationState::Paused
        )
    }

    /// Check if this animation has finished.
    pub fn is_finished(&self) -> bool {
        self.state == AnimationState::Finished
    }

    /// Get the progress through all iterations (0.0 to iteration_count).
    pub fn total_progress(&self) -> f32 {
        self.current_iteration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_fade_animation() -> KeyframeAnimation {
        KeyframeAnimation::new("fade")
            .duration_ms(100.0)
            .keyframe(0.0, |kf| kf.set(AnimatableProperty::Opacity, AnimatableValue::F64 { value: 0.0 }))
            .keyframe(1.0, |kf| kf.set(AnimatableProperty::Opacity, AnimatableValue::F64 { value: 1.0 }))
    }

    #[test]
    fn test_iteration_count() {
        assert!(IterationCount::Infinite.should_continue(1000.0));
        assert!(IterationCount::Count { count: 3.0 }.should_continue(2.5));
        assert!(!IterationCount::Count { count: 3.0 }.should_continue(3.0));
        assert!(!IterationCount::Count { count: 3.0 }.should_continue(3.5));
    }

    #[test]
    fn test_animation_direction() {
        assert!(!AnimationDirection::Normal.is_reversed(0));
        assert!(!AnimationDirection::Normal.is_reversed(1));

        assert!(AnimationDirection::Reverse.is_reversed(0));
        assert!(AnimationDirection::Reverse.is_reversed(1));

        assert!(!AnimationDirection::Alternate.is_reversed(0));
        assert!(AnimationDirection::Alternate.is_reversed(1));
        assert!(!AnimationDirection::Alternate.is_reversed(2));

        assert!(AnimationDirection::AlternateReverse.is_reversed(0));
        assert!(!AnimationDirection::AlternateReverse.is_reversed(1));
        assert!(AnimationDirection::AlternateReverse.is_reversed(2));
    }

    #[test]
    fn test_fill_mode() {
        assert!(!AnimationFillMode::None.applies_backwards());
        assert!(!AnimationFillMode::None.applies_forwards());

        assert!(!AnimationFillMode::Forwards.applies_backwards());
        assert!(AnimationFillMode::Forwards.applies_forwards());

        assert!(AnimationFillMode::Backwards.applies_backwards());
        assert!(!AnimationFillMode::Backwards.applies_forwards());

        assert!(AnimationFillMode::Both.applies_backwards());
        assert!(AnimationFillMode::Both.applies_forwards());
    }

    #[test]
    fn test_keyframe_creation() {
        let kf = Keyframe::new(0.5)
            .set(AnimatableProperty::Opacity, AnimatableValue::F64 { value: 0.5 })
            .with_easing(EasingFunction::EaseOut);

        assert_eq!(kf.offset, 0.5);
        assert_eq!(
            kf.get(AnimatableProperty::Opacity).unwrap().as_f64(),
            Some(0.5)
        );
        assert_eq!(kf.easing, Some(EasingFunction::EaseOut));
    }

    #[test]
    fn test_keyframe_animation_builder() {
        let anim = KeyframeAnimation::new("test")
            .duration_ms(500.0)
            .delay_ms(100.0)
            .iterations(IterationCount::Count { count: 3.0 })
            .direction(AnimationDirection::Alternate)
            .fill_mode(AnimationFillMode::Both)
            .keyframe(0.0, |kf| kf.set(AnimatableProperty::Opacity, AnimatableValue::F64 { value: 0.0 }))
            .keyframe(0.5, |kf| kf.set(AnimatableProperty::Opacity, AnimatableValue::F64 { value: 0.8 }))
            .keyframe(1.0, |kf| kf.set(AnimatableProperty::Opacity, AnimatableValue::F64 { value: 1.0 }));

        assert_eq!(anim.name, "test");
        assert_eq!(anim.duration_ms, 500.0);
        assert_eq!(anim.delay_ms, 100.0);
        assert_eq!(anim.iteration_count, IterationCount::Count { count: 3.0 });
        assert_eq!(anim.direction, AnimationDirection::Alternate);
        assert_eq!(anim.fill_mode, AnimationFillMode::Both);
        assert_eq!(anim.keyframes.len(), 3);

        // Keyframes should be sorted
        assert_eq!(anim.keyframes[0].offset, 0.0);
        assert_eq!(anim.keyframes[1].offset, 0.5);
        assert_eq!(anim.keyframes[2].offset, 1.0);
    }

    #[test]
    fn test_keyframe_animation_value_at() {
        let anim = KeyframeAnimation::new("fade")
            .duration_ms(100.0)
            .default_easing(EasingFunction::Linear)
            .keyframe(0.0, |kf| kf.set(AnimatableProperty::Opacity, AnimatableValue::F64 { value: 0.0 }))
            .keyframe(1.0, |kf| kf.set(AnimatableProperty::Opacity, AnimatableValue::F64 { value: 100.0 }));

        let v = anim
            .value_at(AnimatableProperty::Opacity, 0.0)
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((v - 0.0).abs() < 0.1);

        let v = anim
            .value_at(AnimatableProperty::Opacity, 0.5)
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((v - 50.0).abs() < 0.1);

        let v = anim
            .value_at(AnimatableProperty::Opacity, 1.0)
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((v - 100.0).abs() < 0.1);
    }

    #[test]
    fn test_active_animation_lifecycle() {
        let anim = create_fade_animation();
        let mut active = ActiveKeyframeAnimation::new("node1".to_string(), anim);

        assert_eq!(active.state, AnimationState::Running);
        assert!(active.is_active());

        // Progress to 50%
        active.update(50.0);
        assert_eq!(active.state, AnimationState::Running);
        let offset = active.current_offset();
        assert!((offset - 0.5).abs() < 0.1);

        // Complete
        active.update(60.0);
        assert_eq!(active.state, AnimationState::Finished);
        assert!(!active.is_active());
        assert!(active.is_finished());
    }

    #[test]
    fn test_active_animation_with_delay() {
        let anim = create_fade_animation().delay_ms(50.0);
        let mut active = ActiveKeyframeAnimation::new("node1".to_string(), anim);

        assert_eq!(active.state, AnimationState::Pending);

        // During delay
        active.update(25.0);
        assert_eq!(active.state, AnimationState::Pending);

        // After delay
        active.update(30.0);
        assert_eq!(active.state, AnimationState::Running);
    }

    #[test]
    fn test_active_animation_reverse() {
        let anim = create_fade_animation()
            .direction(AnimationDirection::Reverse)
            .default_easing(EasingFunction::Linear);
        let mut active = ActiveKeyframeAnimation::new("node1".to_string(), anim);

        // At start, should be at end of animation (reversed)
        let offset = active.current_offset();
        assert!((offset - 1.0).abs() < 0.1);

        // At 50%, should be at 0.5 (reversed from 0.5 = 0.5)
        active.update(50.0);
        let offset = active.current_offset();
        assert!((offset - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_active_animation_alternate() {
        let anim = create_fade_animation()
            .duration_ms(100.0)
            .iterations(IterationCount::Count { count: 2.0 })
            .direction(AnimationDirection::Alternate);
        let mut active = ActiveKeyframeAnimation::new("node1".to_string(), anim);

        // First iteration: forward
        active.update(50.0);
        assert!(!active.animation.direction.is_reversed(0));

        // Second iteration: backward
        active.update(60.0);
        assert!(active.animation.direction.is_reversed(1));
    }

    #[test]
    fn test_active_animation_pause_resume() {
        let anim = create_fade_animation();
        let mut active = ActiveKeyframeAnimation::new("node1".to_string(), anim);

        active.update(50.0);
        let offset_before = active.current_offset();

        active.pause();
        assert_eq!(active.play_state, AnimationPlayState::Paused);

        // Time passes but animation doesn't progress
        active.update(100.0);
        let offset_after = active.current_offset();
        assert!((offset_before - offset_after).abs() < 0.01);

        active.resume();
        assert_eq!(active.play_state, AnimationPlayState::Running);
    }

    #[test]
    fn test_active_animation_fill_backwards() {
        let anim = create_fade_animation()
            .delay_ms(100.0)
            .fill_mode(AnimationFillMode::Backwards);
        let active = ActiveKeyframeAnimation::new("node1".to_string(), anim);

        // During delay with backwards fill, should get first keyframe value
        let v = active
            .current_value(AnimatableProperty::Opacity)
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((v - 0.0).abs() < 0.1);
    }

    #[test]
    fn test_active_animation_fill_forwards() {
        let anim = create_fade_animation().fill_mode(AnimationFillMode::Forwards);
        let mut active = ActiveKeyframeAnimation::new("node1".to_string(), anim);

        // Complete the animation
        active.update(150.0);
        assert!(active.is_finished());

        // After completion with forwards fill, should retain final value
        let v = active
            .current_value(AnimatableProperty::Opacity)
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((v - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_infinite_iterations() {
        let anim = create_fade_animation().iterations(IterationCount::Infinite);
        let mut active = ActiveKeyframeAnimation::new("node1".to_string(), anim);

        // Run for many iterations
        active.update(1000.0);
        assert!(active.is_active());
        assert!(!active.is_finished());
    }

    #[test]
    fn test_animated_properties() {
        let anim = KeyframeAnimation::new("multi")
            .keyframe(0.0, |kf| {
                kf.set(AnimatableProperty::Opacity, AnimatableValue::F64 { value: 0.0 })
                    .set(AnimatableProperty::Width, AnimatableValue::F64 { value: 100.0 })
            })
            .keyframe(1.0, |kf| {
                kf.set(AnimatableProperty::Opacity, AnimatableValue::F64 { value: 1.0 })
                    .set(AnimatableProperty::Width, AnimatableValue::F64 { value: 200.0 })
                    .set(AnimatableProperty::Height, AnimatableValue::F64 { value: 150.0 })
            });

        let props = anim.animated_properties();
        assert!(props.contains(&AnimatableProperty::Opacity));
        assert!(props.contains(&AnimatableProperty::Width));
        assert!(props.contains(&AnimatableProperty::Height));
    }

    #[test]
    fn test_current_values() {
        let anim = KeyframeAnimation::new("multi")
            .duration_ms(100.0)
            .default_easing(EasingFunction::Linear)
            .keyframe(0.0, |kf| {
                kf.set(AnimatableProperty::Opacity, AnimatableValue::F64 { value: 0.0 })
                    .set(AnimatableProperty::Width, AnimatableValue::F64 { value: 100.0 })
            })
            .keyframe(1.0, |kf| {
                kf.set(AnimatableProperty::Opacity, AnimatableValue::F64 { value: 1.0 })
                    .set(AnimatableProperty::Width, AnimatableValue::F64 { value: 200.0 })
            });

        let mut active = ActiveKeyframeAnimation::new("node1".to_string(), anim);
        active.update(50.0);

        let values = active.current_values();
        assert_eq!(values.len(), 2);

        let opacity = values
            .get(&AnimatableProperty::Opacity)
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((opacity - 0.5).abs() < 0.1);

        let width = values
            .get(&AnimatableProperty::Width)
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((width - 150.0).abs() < 1.0);
    }
}
