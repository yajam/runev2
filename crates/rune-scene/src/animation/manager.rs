//! Animation manager for coordinating transitions and keyframe animations.
//!
//! The `AnimationManager` is the central coordinator for all animations in the
//! scene. It handles:
//! - Starting and tracking transitions (property value changes)
//! - Starting and tracking keyframe animations (multi-step sequences)
//! - Updating all active animations each frame
//! - Providing current animated values for rendering
//! - Handling transition interruption (retargeting)
//!
//! # Usage
//!
//! ```ignore
//! use rune_scene::animation::manager::AnimationManager;
//! use rune_scene::animation::transition::TransitionSpec;
//! use rune_scene::animation::keyframes::KeyframeAnimation;
//!
//! let mut manager = AnimationManager::new();
//!
//! // Start a transition
//! let id = manager.start_transition(
//!     "node1",
//!     AnimatableProperty::Opacity,
//!     AnimatableValue::F64 { value: 0.0 },
//!     AnimatableValue::F64 { value: 1.0 },
//!     &TransitionSpec::all(300.0),
//! );
//!
//! // Or start a keyframe animation
//! let anim = KeyframeAnimation::new("fade-in")
//!     .duration_ms(500.0)
//!     .keyframe(0.0, |kf| kf.set(AnimatableProperty::Opacity, AnimatableValue::F64 { value: 0.0 }))
//!     .keyframe(1.0, |kf| kf.set(AnimatableProperty::Opacity, AnimatableValue::F64 { value: 1.0 }));
//! let id = manager.start_keyframe_animation("node1", anim);
//!
//! // Each frame, update and get values
//! manager.update(16.67); // ~60fps
//! if let Some(value) = manager.get_animated_value("node1", AnimatableProperty::Opacity) {
//!     // Use animated value for rendering
//! }
//! ```

use std::collections::HashMap;

use super::events::{AnimationEvent, AnimationEventKind, EventQueue, TransitionEvent};
use super::keyframes::{ActiveKeyframeAnimation, KeyframeAnimation};
use super::transition::{ActiveTransition, TransitionSpec};
use super::types::{AnimatableProperty, AnimatableValue, AnimationId, AnimationState};

/// Central manager for all active animations in the scene.
///
/// Thread safety: This struct is `Send` for potential future async usage.
#[derive(Debug, Default)]
pub struct AnimationManager {
    /// Active transitions indexed by their ID.
    transitions: HashMap<AnimationId, ActiveTransition>,

    /// Active keyframe animations indexed by their ID.
    keyframe_animations: HashMap<AnimationId, ActiveKeyframeAnimation>,

    /// Index from (node_id, property) to transition ID for quick lookup.
    /// Only one transition can be active per (node, property) pair.
    node_property_index: HashMap<(String, AnimatableProperty), AnimationId>,

    /// Index from node_id to keyframe animation IDs.
    /// Multiple keyframe animations can be active per node.
    node_keyframe_index: HashMap<String, Vec<AnimationId>>,

    /// Registry of named keyframe animations for reuse.
    animation_registry: HashMap<String, KeyframeAnimation>,

    /// Flag indicating if any animations changed this frame.
    dirty: bool,

    /// Queue of animation events emitted during updates.
    event_queue: EventQueue,
}

impl AnimationManager {
    /// Create a new animation manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a new transition for a property.
    ///
    /// If a transition is already running for this (node, property) pair,
    /// the existing transition will be retargeted to the new value.
    ///
    /// # Arguments
    /// * `node_id` - The ID of the node being animated
    /// * `property` - The property being transitioned
    /// * `from_value` - Starting value
    /// * `to_value` - Target value
    /// * `spec` - Transition specification (duration, easing, etc.)
    ///
    /// # Returns
    /// The animation ID for the new or retargeted transition.
    pub fn start_transition(
        &mut self,
        node_id: &str,
        property: AnimatableProperty,
        from_value: AnimatableValue,
        to_value: AnimatableValue,
        spec: &TransitionSpec,
    ) -> AnimationId {
        let key = (node_id.to_string(), property);

        // Check if there's an existing transition for this node/property
        if let Some(&existing_id) = self.node_property_index.get(&key) {
            if let Some(existing) = self.transitions.get_mut(&existing_id) {
                // Retarget the existing transition
                existing.retarget(to_value, spec);
                self.dirty = true;
                return existing_id;
            }
        }

        // Create a new transition
        let transition =
            ActiveTransition::new(node_id.to_string(), property, from_value, to_value, spec);
        let id = transition.id;

        // Emit started event
        self.event_queue
            .push_transition_event(TransitionEvent::Started {
                transition_id: id,
                node_id: node_id.to_string(),
                property,
            });

        self.transitions.insert(id, transition);
        self.node_property_index.insert(key, id);
        self.dirty = true;

        id
    }

    /// Update all active animations by the given delta time.
    ///
    /// This should be called once per frame with the elapsed time in milliseconds.
    /// Finished animations are automatically cleaned up.
    pub fn update(&mut self, delta_ms: f32) {
        let has_transitions = !self.transitions.is_empty();
        let has_keyframes = !self.keyframe_animations.is_empty();

        if !has_transitions && !has_keyframes {
            return;
        }

        // Update transitions
        let mut finished_transition_ids = Vec::new();
        for (id, transition) in self.transitions.iter_mut() {
            if !transition.update(delta_ms) {
                finished_transition_ids.push(*id);
            }
        }

        // Clean up finished transitions and emit events
        for id in finished_transition_ids {
            if let Some(transition) = self.transitions.remove(&id) {
                let key = (transition.node_id.clone(), transition.property);
                self.node_property_index.remove(&key);

                // Emit ended or cancelled event based on state
                let event = if transition.state == AnimationState::Cancelled {
                    TransitionEvent::Cancelled {
                        transition_id: id,
                        node_id: transition.node_id,
                        property: transition.property,
                    }
                } else {
                    TransitionEvent::Ended {
                        transition_id: id,
                        node_id: transition.node_id,
                        property: transition.property,
                    }
                };
                self.event_queue.push_transition_event(event);
            }
        }

        // Update keyframe animations
        let mut finished_keyframe_ids = Vec::new();
        for (id, animation) in self.keyframe_animations.iter_mut() {
            if !animation.update(delta_ms) {
                finished_keyframe_ids.push(*id);
            }
        }

        // Clean up finished keyframe animations and emit events
        for id in finished_keyframe_ids {
            if let Some(animation) = self.keyframe_animations.remove(&id) {
                if let Some(ids) = self.node_keyframe_index.get_mut(&animation.node_id) {
                    ids.retain(|i| *i != id);
                    if ids.is_empty() {
                        self.node_keyframe_index.remove(&animation.node_id);
                    }
                }

                // Emit ended or cancelled event based on state
                let event = if animation.state == AnimationState::Cancelled {
                    AnimationEvent::Cancelled {
                        animation_id: id,
                        node_id: animation.node_id,
                        animation_name: animation.animation.name.clone(),
                    }
                } else {
                    AnimationEvent::Ended {
                        animation_id: id,
                        node_id: animation.node_id,
                        animation_name: animation.animation.name.clone(),
                    }
                };
                self.event_queue.push_animation_event(event);
            }
        }

        self.dirty = !self.transitions.is_empty() || !self.keyframe_animations.is_empty();
    }

    /// Get the current animated value for a property.
    ///
    /// Checks transitions first, then keyframe animations.
    /// Returns `None` if no animation is active for this property.
    pub fn get_animated_value(
        &self,
        node_id: &str,
        property: AnimatableProperty,
    ) -> Option<AnimatableValue> {
        // First check transitions (they take priority)
        let key = (node_id.to_string(), property);
        if let Some(id) = self.node_property_index.get(&key) {
            if let Some(transition) = self.transitions.get(id) {
                return Some(transition.current_value());
            }
        }

        // Then check keyframe animations
        if let Some(ids) = self.node_keyframe_index.get(node_id) {
            for id in ids {
                if let Some(animation) = self.keyframe_animations.get(id) {
                    if let Some(value) = animation.current_value(property) {
                        return Some(value);
                    }
                }
            }
        }

        None
    }

    /// Get all animated values for a node.
    ///
    /// Combines values from both transitions and keyframe animations.
    /// Transitions take priority over keyframe animations for the same property.
    pub fn get_all_animated_values(
        &self,
        node_id: &str,
    ) -> HashMap<AnimatableProperty, AnimatableValue> {
        let mut values = HashMap::new();

        // Get keyframe animation values first (lower priority)
        if let Some(ids) = self.node_keyframe_index.get(node_id) {
            for id in ids {
                if let Some(animation) = self.keyframe_animations.get(id) {
                    for (prop, value) in animation.current_values() {
                        values.insert(prop, value);
                    }
                }
            }
        }

        // Get transition values (higher priority, overwrites keyframe values)
        for ((nid, prop), id) in &self.node_property_index {
            if nid == node_id {
                if let Some(transition) = self.transitions.get(id) {
                    values.insert(*prop, transition.current_value());
                }
            }
        }

        values
    }

    // ========================================================================
    // Keyframe Animation Methods
    // ========================================================================

    /// Register a named keyframe animation for later use.
    pub fn register_animation(&mut self, animation: KeyframeAnimation) {
        self.animation_registry
            .insert(animation.name.clone(), animation);
    }

    /// Get a registered animation by name.
    pub fn get_registered_animation(&self, name: &str) -> Option<&KeyframeAnimation> {
        self.animation_registry.get(name)
    }

    /// Start a keyframe animation on a node.
    pub fn start_keyframe_animation(
        &mut self,
        node_id: &str,
        animation: KeyframeAnimation,
    ) -> AnimationId {
        let animation_name = animation.name.clone();
        let active = ActiveKeyframeAnimation::new(node_id.to_string(), animation);
        let id = active.id;

        // Emit started event
        self.event_queue
            .push_animation_event(AnimationEvent::Started {
                animation_id: id,
                node_id: node_id.to_string(),
                animation_name,
            });

        self.keyframe_animations.insert(id, active);
        self.node_keyframe_index
            .entry(node_id.to_string())
            .or_default()
            .push(id);
        self.dirty = true;

        id
    }

    /// Start a registered animation by name on a node.
    ///
    /// Returns `None` if the animation name is not registered.
    pub fn start_registered_animation(
        &mut self,
        node_id: &str,
        animation_name: &str,
    ) -> Option<AnimationId> {
        let animation = self.animation_registry.get(animation_name)?.clone();
        Some(self.start_keyframe_animation(node_id, animation))
    }

    /// Cancel a keyframe animation by ID.
    pub fn cancel_keyframe_animation(&mut self, id: AnimationId) {
        if let Some(animation) = self.keyframe_animations.get_mut(&id) {
            animation.cancel();
        }
    }

    /// Cancel all keyframe animations for a node.
    pub fn cancel_keyframe_animations_for_node(&mut self, node_id: &str) {
        if let Some(ids) = self.node_keyframe_index.get(node_id) {
            for id in ids.clone() {
                self.cancel_keyframe_animation(id);
            }
        }
    }

    /// Pause a keyframe animation.
    pub fn pause_keyframe_animation(&mut self, id: AnimationId) {
        if let Some(animation) = self.keyframe_animations.get_mut(&id) {
            animation.pause();
        }
    }

    /// Resume a keyframe animation.
    pub fn resume_keyframe_animation(&mut self, id: AnimationId) {
        if let Some(animation) = self.keyframe_animations.get_mut(&id) {
            animation.resume();
        }
    }

    /// Get a reference to an active keyframe animation by ID.
    pub fn get_keyframe_animation(&self, id: AnimationId) -> Option<&ActiveKeyframeAnimation> {
        self.keyframe_animations.get(&id)
    }

    /// Get the number of active keyframe animations.
    pub fn keyframe_animation_count(&self) -> usize {
        self.keyframe_animations
            .values()
            .filter(|a| a.is_active())
            .count()
    }

    /// Cancel a specific transition by ID.
    pub fn cancel_transition(&mut self, id: AnimationId) {
        if let Some(transition) = self.transitions.get_mut(&id) {
            transition.cancel();
        }
    }

    /// Cancel all transitions for a specific node.
    pub fn cancel_all_for_node(&mut self, node_id: &str) {
        let ids_to_cancel: Vec<AnimationId> = self
            .node_property_index
            .iter()
            .filter(|((nid, _), _)| nid == node_id)
            .map(|(_, id)| *id)
            .collect();

        for id in ids_to_cancel {
            self.cancel_transition(id);
        }
    }

    /// Cancel a transition for a specific node and property.
    pub fn cancel_transition_for(&mut self, node_id: &str, property: AnimatableProperty) {
        let key = (node_id.to_string(), property);
        if let Some(&id) = self.node_property_index.get(&key) {
            self.cancel_transition(id);
        }
    }

    /// Pause a specific transition.
    pub fn pause_transition(&mut self, id: AnimationId) {
        if let Some(transition) = self.transitions.get_mut(&id) {
            transition.pause();
        }
    }

    /// Resume a paused transition.
    pub fn resume_transition(&mut self, id: AnimationId) {
        if let Some(transition) = self.transitions.get_mut(&id) {
            transition.resume();
        }
    }

    /// Pause all transitions for a node.
    pub fn pause_all_for_node(&mut self, node_id: &str) {
        for ((nid, _), id) in &self.node_property_index {
            if nid == node_id {
                if let Some(transition) = self.transitions.get_mut(id) {
                    transition.pause();
                }
            }
        }
    }

    /// Resume all transitions for a node.
    pub fn resume_all_for_node(&mut self, node_id: &str) {
        for ((nid, _), id) in &self.node_property_index {
            if nid == node_id {
                if let Some(transition) = self.transitions.get_mut(id) {
                    transition.resume();
                }
            }
        }
    }

    /// Check if any animations are currently active.
    pub fn has_active_animations(&self) -> bool {
        self.transitions.values().any(|t| t.is_active())
            || self.keyframe_animations.values().any(|a| a.is_active())
    }

    /// Get the number of active transitions.
    pub fn active_transition_count(&self) -> usize {
        self.transitions.values().filter(|t| t.is_active()).count()
    }

    /// Get the total number of active animations (transitions + keyframe animations).
    pub fn active_count(&self) -> usize {
        self.active_transition_count() + self.keyframe_animation_count()
    }

    /// Check if animations need a redraw.
    pub fn needs_redraw(&self) -> bool {
        self.dirty && (!self.transitions.is_empty() || !self.keyframe_animations.is_empty())
    }

    /// Check if any layout-affecting properties are being animated.
    ///
    /// This is useful for determining whether a Taffy relayout is needed.
    pub fn has_layout_animations(&self) -> bool {
        // Check transitions
        for ((_, prop), id) in &self.node_property_index {
            if prop.affects_layout() {
                if let Some(transition) = self.transitions.get(id) {
                    if transition.is_active() {
                        return true;
                    }
                }
            }
        }

        // Check keyframe animations
        for active in self.keyframe_animations.values() {
            if active.is_active() {
                for prop in active.animation.animated_properties() {
                    if prop.affects_layout() {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Clear the dirty flag (call after redraw).
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Get a reference to an active transition by ID.
    pub fn get_transition(&self, id: AnimationId) -> Option<&ActiveTransition> {
        self.transitions.get(&id)
    }

    /// Remove all finished and cancelled animations.
    ///
    /// This is called automatically by `update()`, but can be called manually
    /// if needed.
    pub fn cleanup(&mut self) {
        // Cleanup transitions
        let to_remove: Vec<AnimationId> = self
            .transitions
            .iter()
            .filter(|(_, t)| !t.is_active())
            .map(|(id, _)| *id)
            .collect();

        for id in to_remove {
            if let Some(transition) = self.transitions.remove(&id) {
                let key = (transition.node_id.clone(), transition.property);
                self.node_property_index.remove(&key);
            }
        }

        // Cleanup keyframe animations
        let to_remove: Vec<AnimationId> = self
            .keyframe_animations
            .iter()
            .filter(|(_, a)| !a.is_active())
            .map(|(id, _)| *id)
            .collect();

        for id in to_remove {
            if let Some(animation) = self.keyframe_animations.remove(&id) {
                if let Some(ids) = self.node_keyframe_index.get_mut(&animation.node_id) {
                    ids.retain(|i| *i != id);
                    if ids.is_empty() {
                        self.node_keyframe_index.remove(&animation.node_id);
                    }
                }
            }
        }
    }

    /// Clear all animations (but preserve the registry).
    pub fn clear(&mut self) {
        self.transitions.clear();
        self.keyframe_animations.clear();
        self.node_property_index.clear();
        self.node_keyframe_index.clear();
        self.dirty = false;
    }

    /// Clear all animations including the registry.
    pub fn clear_all(&mut self) {
        self.clear();
        self.animation_registry.clear();
    }

    // ========================================================================
    // Event Methods
    // ========================================================================

    /// Drain all pending events from the queue.
    ///
    /// Returns an iterator over all events that occurred since the last drain.
    /// Events are removed from the queue as they are yielded.
    ///
    /// # Example
    /// ```ignore
    /// for event in manager.drain_events() {
    ///     match event {
    ///         AnimationEventKind::Transition(TransitionEvent::Ended { node_id, .. }) => {
    ///             println!("Transition ended for {}", node_id);
    ///         }
    ///         AnimationEventKind::Keyframe(AnimationEvent::Started { animation_name, .. }) => {
    ///             println!("Animation {} started", animation_name);
    ///         }
    ///         _ => {}
    ///     }
    /// }
    /// ```
    pub fn drain_events(&mut self) -> impl Iterator<Item = AnimationEventKind> + '_ {
        self.event_queue.drain()
    }

    /// Check if there are any pending events.
    pub fn has_pending_events(&self) -> bool {
        !self.event_queue.is_empty()
    }

    /// Get the number of pending events.
    pub fn pending_event_count(&self) -> usize {
        self.event_queue.len()
    }

    /// Peek at the next event without removing it.
    pub fn peek_event(&self) -> Option<&AnimationEventKind> {
        self.event_queue.peek()
    }

    /// Pop a single event from the queue.
    pub fn pop_event(&mut self) -> Option<AnimationEventKind> {
        self.event_queue.pop()
    }

    /// Get all pending events for a specific node (without removing them).
    pub fn events_for_node(&self, node_id: &str) -> Vec<&AnimationEventKind> {
        self.event_queue.events_for_node(node_id)
    }

    /// Clear all pending events without processing them.
    pub fn clear_events(&mut self) {
        self.event_queue.clear();
    }
}

// Ensure AnimationManager is Send for async compatibility
static_assertions::assert_impl_all!(AnimationManager: Send);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::easing::EasingFunction;

    #[test]
    fn test_start_transition() {
        let mut manager = AnimationManager::new();

        let id = manager.start_transition(
            "node1",
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 1.0 },
            &TransitionSpec::all(100.0),
        );

        assert!(manager.has_active_animations());
        assert_eq!(manager.active_count(), 1);
        assert!(manager.get_transition(id).is_some());
    }

    #[test]
    fn test_get_animated_value() {
        let mut manager = AnimationManager::new();

        manager.start_transition(
            "node1",
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 100.0 },
            &TransitionSpec::all(100.0).with_easing(EasingFunction::Linear),
        );

        // Initial value
        let v = manager
            .get_animated_value("node1", AnimatableProperty::Opacity)
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((v - 0.0).abs() < 0.1);

        // After 50% progress
        manager.update(50.0);
        let v = manager
            .get_animated_value("node1", AnimatableProperty::Opacity)
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((v - 50.0).abs() < 1.0);

        // No animation for other properties
        assert!(manager
            .get_animated_value("node1", AnimatableProperty::Width)
            .is_none());
    }

    #[test]
    fn test_transition_completion() {
        let mut manager = AnimationManager::new();

        manager.start_transition(
            "node1",
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 1.0 },
            &TransitionSpec::all(100.0),
        );

        assert!(manager.has_active_animations());

        // Complete the transition
        manager.update(150.0);

        // Transition should be cleaned up
        assert!(!manager.has_active_animations());
        assert_eq!(manager.active_count(), 0);
        assert!(manager
            .get_animated_value("node1", AnimatableProperty::Opacity)
            .is_none());
    }

    #[test]
    fn test_transition_retargeting() {
        let mut manager = AnimationManager::new();

        let id1 = manager.start_transition(
            "node1",
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 100.0 },
            &TransitionSpec::all(100.0).with_easing(EasingFunction::Linear),
        );

        // Progress to 50%
        manager.update(50.0);

        // Start a new transition to the same property (should retarget)
        let id2 = manager.start_transition(
            "node1",
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 50.0 }, // This will be ignored, uses current value
            AnimatableValue::F64 { value: 0.0 },  // New target
            &TransitionSpec::all(100.0).with_easing(EasingFunction::Linear),
        );

        // Should reuse the same transition
        assert_eq!(id1, id2);
        assert_eq!(manager.active_count(), 1);

        // Current value should be around 50 (was at 50% of 0->100)
        let v = manager
            .get_animated_value("node1", AnimatableProperty::Opacity)
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((v - 50.0).abs() < 1.0);
    }

    #[test]
    fn test_cancel_transition() {
        let mut manager = AnimationManager::new();

        let id = manager.start_transition(
            "node1",
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 1.0 },
            &TransitionSpec::all(100.0),
        );

        manager.cancel_transition(id);
        manager.cleanup();

        assert!(!manager.has_active_animations());
    }

    #[test]
    fn test_cancel_all_for_node() {
        let mut manager = AnimationManager::new();

        manager.start_transition(
            "node1",
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 1.0 },
            &TransitionSpec::all(100.0),
        );

        manager.start_transition(
            "node1",
            AnimatableProperty::Width,
            AnimatableValue::F64 { value: 100.0 },
            AnimatableValue::F64 { value: 200.0 },
            &TransitionSpec::all(100.0),
        );

        manager.start_transition(
            "node2",
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 1.0 },
            &TransitionSpec::all(100.0),
        );

        assert_eq!(manager.active_count(), 3);

        manager.cancel_all_for_node("node1");
        manager.cleanup();

        // Only node2's transition should remain
        assert_eq!(manager.active_count(), 1);
        assert!(manager
            .get_animated_value("node2", AnimatableProperty::Opacity)
            .is_some());
    }

    #[test]
    fn test_multiple_properties() {
        let mut manager = AnimationManager::new();

        manager.start_transition(
            "node1",
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 1.0 },
            &TransitionSpec::all(100.0),
        );

        manager.start_transition(
            "node1",
            AnimatableProperty::Width,
            AnimatableValue::F64 { value: 100.0 },
            AnimatableValue::F64 { value: 200.0 },
            &TransitionSpec::all(100.0),
        );

        assert_eq!(manager.active_count(), 2);

        let values = manager.get_all_animated_values("node1");
        assert_eq!(values.len(), 2);
        assert!(values.contains_key(&AnimatableProperty::Opacity));
        assert!(values.contains_key(&AnimatableProperty::Width));
    }

    #[test]
    fn test_pause_resume() {
        let mut manager = AnimationManager::new();

        let id = manager.start_transition(
            "node1",
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 100.0 },
            &TransitionSpec::all(100.0).with_easing(EasingFunction::Linear),
        );

        manager.update(50.0);
        let v_before = manager
            .get_animated_value("node1", AnimatableProperty::Opacity)
            .unwrap()
            .as_f64()
            .unwrap();

        manager.pause_transition(id);
        manager.update(50.0); // Time passes but paused

        let v_after = manager
            .get_animated_value("node1", AnimatableProperty::Opacity)
            .unwrap()
            .as_f64()
            .unwrap();

        assert!((v_before - v_after).abs() < 0.1);

        manager.resume_transition(id);
        manager.update(25.0);

        let v_resumed = manager
            .get_animated_value("node1", AnimatableProperty::Opacity)
            .unwrap()
            .as_f64()
            .unwrap();

        assert!(v_resumed > v_before);
    }

    #[test]
    fn test_needs_redraw() {
        let mut manager = AnimationManager::new();

        assert!(!manager.needs_redraw());

        manager.start_transition(
            "node1",
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 1.0 },
            &TransitionSpec::all(100.0),
        );

        assert!(manager.needs_redraw());

        manager.clear_dirty();
        assert!(!manager.needs_redraw());

        manager.update(10.0);
        assert!(manager.needs_redraw());
    }

    // ========================================================================
    // Keyframe Animation Tests
    // ========================================================================

    fn create_fade_animation() -> KeyframeAnimation {
        KeyframeAnimation::new("fade")
            .duration_ms(100.0)
            .default_easing(EasingFunction::Linear)
            .keyframe(0.0, |kf| kf.set(AnimatableProperty::Opacity, AnimatableValue::F64 { value: 0.0 }))
            .keyframe(1.0, |kf| kf.set(AnimatableProperty::Opacity, AnimatableValue::F64 { value: 100.0 }))
    }

    #[test]
    fn test_start_keyframe_animation() {
        let mut manager = AnimationManager::new();

        let anim = create_fade_animation();
        let id = manager.start_keyframe_animation("node1", anim);

        assert!(manager.has_active_animations());
        assert_eq!(manager.keyframe_animation_count(), 1);
        assert!(manager.get_keyframe_animation(id).is_some());
    }

    #[test]
    fn test_keyframe_animation_value() {
        let mut manager = AnimationManager::new();

        let anim = create_fade_animation();
        manager.start_keyframe_animation("node1", anim);

        // Initial value
        let v = manager
            .get_animated_value("node1", AnimatableProperty::Opacity)
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((v - 0.0).abs() < 1.0);

        // After 50% progress
        manager.update(50.0);
        let v = manager
            .get_animated_value("node1", AnimatableProperty::Opacity)
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((v - 50.0).abs() < 1.0);
    }

    #[test]
    fn test_keyframe_animation_completion() {
        let mut manager = AnimationManager::new();

        let anim = create_fade_animation();
        manager.start_keyframe_animation("node1", anim);

        assert!(manager.has_active_animations());

        // Complete the animation
        manager.update(150.0);

        // Animation should be cleaned up
        assert!(!manager.has_active_animations());
        assert_eq!(manager.keyframe_animation_count(), 0);
    }

    #[test]
    fn test_register_and_start_animation() {
        let mut manager = AnimationManager::new();

        // Register animation
        let anim = create_fade_animation();
        manager.register_animation(anim);

        // Start by name
        let id = manager.start_registered_animation("node1", "fade");
        assert!(id.is_some());
        assert!(manager.has_active_animations());

        // Unknown name returns None
        let id = manager.start_registered_animation("node2", "unknown");
        assert!(id.is_none());
    }

    #[test]
    fn test_transition_priority_over_keyframe() {
        let mut manager = AnimationManager::new();

        // Start keyframe animation
        let anim = create_fade_animation();
        manager.start_keyframe_animation("node1", anim);

        // Start transition on same property
        manager.start_transition(
            "node1",
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 200.0 },
            AnimatableValue::F64 { value: 300.0 },
            &TransitionSpec::all(100.0).with_easing(EasingFunction::Linear),
        );

        // Transition should take priority
        let v = manager
            .get_animated_value("node1", AnimatableProperty::Opacity)
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((v - 200.0).abs() < 1.0);
    }

    #[test]
    fn test_multiple_keyframe_animations() {
        let mut manager = AnimationManager::new();

        let fade = create_fade_animation();
        let slide = KeyframeAnimation::new("slide")
            .duration_ms(100.0)
            .default_easing(EasingFunction::Linear)
            .keyframe(0.0, |kf| kf.set(AnimatableProperty::TranslateX, AnimatableValue::F64 { value: 0.0 }))
            .keyframe(1.0, |kf| {
                kf.set(AnimatableProperty::TranslateX, AnimatableValue::F64 { value: 100.0 })
            });

        manager.start_keyframe_animation("node1", fade);
        manager.start_keyframe_animation("node1", slide);

        assert_eq!(manager.keyframe_animation_count(), 2);

        let values = manager.get_all_animated_values("node1");
        assert!(values.contains_key(&AnimatableProperty::Opacity));
        assert!(values.contains_key(&AnimatableProperty::TranslateX));
    }

    #[test]
    fn test_cancel_keyframe_animation() {
        let mut manager = AnimationManager::new();

        let anim = create_fade_animation();
        let id = manager.start_keyframe_animation("node1", anim);

        manager.cancel_keyframe_animation(id);
        manager.cleanup();

        assert!(!manager.has_active_animations());
    }

    #[test]
    fn test_cancel_keyframe_animations_for_node() {
        let mut manager = AnimationManager::new();

        manager.start_keyframe_animation("node1", create_fade_animation());
        manager.start_keyframe_animation("node1", create_fade_animation());
        manager.start_keyframe_animation("node2", create_fade_animation());

        assert_eq!(manager.keyframe_animation_count(), 3);

        manager.cancel_keyframe_animations_for_node("node1");
        manager.cleanup();

        assert_eq!(manager.keyframe_animation_count(), 1);
    }

    #[test]
    fn test_pause_resume_keyframe_animation() {
        let mut manager = AnimationManager::new();

        let anim = create_fade_animation();
        let id = manager.start_keyframe_animation("node1", anim);

        manager.update(50.0);
        let v_before = manager
            .get_animated_value("node1", AnimatableProperty::Opacity)
            .unwrap()
            .as_f64()
            .unwrap();

        manager.pause_keyframe_animation(id);
        manager.update(50.0); // Time passes but paused

        let v_after = manager
            .get_animated_value("node1", AnimatableProperty::Opacity)
            .unwrap()
            .as_f64()
            .unwrap();

        assert!((v_before - v_after).abs() < 0.1);

        manager.resume_keyframe_animation(id);
        manager.update(25.0);

        let v_resumed = manager
            .get_animated_value("node1", AnimatableProperty::Opacity)
            .unwrap()
            .as_f64()
            .unwrap();

        assert!(v_resumed > v_before);
    }

    #[test]
    fn test_clear_preserves_registry() {
        let mut manager = AnimationManager::new();

        manager.register_animation(create_fade_animation());
        manager.start_registered_animation("node1", "fade");

        manager.clear();

        assert!(!manager.has_active_animations());
        assert!(manager.get_registered_animation("fade").is_some());
    }

    #[test]
    fn test_clear_all() {
        let mut manager = AnimationManager::new();

        manager.register_animation(create_fade_animation());
        manager.start_registered_animation("node1", "fade");

        manager.clear_all();

        assert!(!manager.has_active_animations());
        assert!(manager.get_registered_animation("fade").is_none());
    }

    // ========================================================================
    // Event Tests
    // ========================================================================

    #[test]
    fn test_transition_started_event() {
        let mut manager = AnimationManager::new();

        let id = manager.start_transition(
            "node1",
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 1.0 },
            &TransitionSpec::all(100.0),
        );

        assert!(manager.has_pending_events());
        assert_eq!(manager.pending_event_count(), 1);

        let event = manager.pop_event().unwrap();
        match event {
            AnimationEventKind::Transition(TransitionEvent::Started {
                transition_id,
                node_id,
                property,
            }) => {
                assert_eq!(transition_id, id);
                assert_eq!(node_id, "node1");
                assert_eq!(property, AnimatableProperty::Opacity);
            }
            _ => panic!("Expected TransitionEvent::Started"),
        }

        assert!(!manager.has_pending_events());
    }

    #[test]
    fn test_transition_ended_event() {
        let mut manager = AnimationManager::new();

        let id = manager.start_transition(
            "node1",
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 1.0 },
            &TransitionSpec::all(100.0),
        );

        // Clear the started event
        manager.clear_events();

        // Complete the transition
        manager.update(150.0);

        assert!(manager.has_pending_events());
        let event = manager.pop_event().unwrap();
        match event {
            AnimationEventKind::Transition(TransitionEvent::Ended {
                transition_id,
                node_id,
                property,
            }) => {
                assert_eq!(transition_id, id);
                assert_eq!(node_id, "node1");
                assert_eq!(property, AnimatableProperty::Opacity);
            }
            _ => panic!("Expected TransitionEvent::Ended"),
        }
    }

    #[test]
    fn test_keyframe_animation_started_event() {
        let mut manager = AnimationManager::new();

        let anim = create_fade_animation();
        let id = manager.start_keyframe_animation("node1", anim);

        assert!(manager.has_pending_events());
        let event = manager.pop_event().unwrap();
        match event {
            AnimationEventKind::Keyframe(AnimationEvent::Started {
                animation_id,
                node_id,
                animation_name,
            }) => {
                assert_eq!(animation_id, id);
                assert_eq!(node_id, "node1");
                assert_eq!(animation_name, "fade");
            }
            _ => panic!("Expected AnimationEvent::Started"),
        }
    }

    #[test]
    fn test_keyframe_animation_ended_event() {
        let mut manager = AnimationManager::new();

        let anim = create_fade_animation();
        let id = manager.start_keyframe_animation("node1", anim);

        // Clear the started event
        manager.clear_events();

        // Complete the animation
        manager.update(150.0);

        assert!(manager.has_pending_events());
        let event = manager.pop_event().unwrap();
        match event {
            AnimationEventKind::Keyframe(AnimationEvent::Ended {
                animation_id,
                node_id,
                animation_name,
            }) => {
                assert_eq!(animation_id, id);
                assert_eq!(node_id, "node1");
                assert_eq!(animation_name, "fade");
            }
            _ => panic!("Expected AnimationEvent::Ended"),
        }
    }

    #[test]
    fn test_drain_events() {
        let mut manager = AnimationManager::new();

        // Start multiple animations
        manager.start_transition(
            "node1",
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 1.0 },
            &TransitionSpec::all(100.0),
        );
        manager.start_keyframe_animation("node2", create_fade_animation());

        assert_eq!(manager.pending_event_count(), 2);

        let events: Vec<_> = manager.drain_events().collect();
        assert_eq!(events.len(), 2);
        assert!(!manager.has_pending_events());
    }

    #[test]
    fn test_events_for_node() {
        let mut manager = AnimationManager::new();

        manager.start_transition(
            "node1",
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 1.0 },
            &TransitionSpec::all(100.0),
        );
        manager.start_transition(
            "node2",
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 1.0 },
            &TransitionSpec::all(100.0),
        );
        manager.start_keyframe_animation("node1", create_fade_animation());

        let node1_events = manager.events_for_node("node1");
        assert_eq!(node1_events.len(), 2);

        let node2_events = manager.events_for_node("node2");
        assert_eq!(node2_events.len(), 1);
    }

    #[test]
    fn test_peek_event() {
        let mut manager = AnimationManager::new();

        manager.start_transition(
            "node1",
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 0.0 },
            AnimatableValue::F64 { value: 1.0 },
            &TransitionSpec::all(100.0),
        );

        // Peek doesn't remove
        assert!(manager.peek_event().is_some());
        assert!(manager.peek_event().is_some());
        assert_eq!(manager.pending_event_count(), 1);

        // Pop removes
        manager.pop_event();
        assert!(manager.peek_event().is_none());
    }
}
