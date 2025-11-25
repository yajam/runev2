//! State change detection for automatic animation triggering.
//!
//! This module provides `StateTracker` which monitors property changes on IR nodes
//! and automatically starts transitions when properties with configured transitions change.
//!
//! # Usage
//!
//! ```ignore
//! use rune_scene::animation::{StateTracker, AnimationManager, TransitionSpec};
//!
//! let mut tracker = StateTracker::new();
//! let mut manager = AnimationManager::new();
//!
//! // Configure transitions for a node
//! tracker.set_transition_spec("button_1", TransitionSpec::all(300.0));
//!
//! // Snapshot current property values
//! tracker.snapshot_properties("button_1", current_props);
//!
//! // Later, when properties might have changed...
//! tracker.detect_and_trigger_transitions("button_1", new_props, &mut manager);
//! ```

use std::collections::HashMap;

use super::manager::AnimationManager;
use super::transition::TransitionSpec;
use super::types::{AnimatableProperty, AnimatableValue};

/// A snapshot of animatable property values for a node.
#[derive(Debug, Clone, Default)]
pub struct PropertySnapshot {
    /// Property values at the time of snapshot.
    pub values: HashMap<AnimatableProperty, AnimatableValue>,
}

impl PropertySnapshot {
    /// Create a new empty snapshot.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a snapshot with the given values.
    pub fn with_values(values: HashMap<AnimatableProperty, AnimatableValue>) -> Self {
        Self { values }
    }

    /// Set a property value.
    pub fn set(&mut self, property: AnimatableProperty, value: AnimatableValue) {
        self.values.insert(property, value);
    }

    /// Get a property value.
    pub fn get(&self, property: AnimatableProperty) -> Option<&AnimatableValue> {
        self.values.get(&property)
    }

    /// Check if the snapshot is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Get the number of properties in the snapshot.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Iterate over all property-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&AnimatableProperty, &AnimatableValue)> {
        self.values.iter()
    }
}

/// Configuration for a node's transition behavior.
#[derive(Debug, Clone)]
pub struct NodeTransitionConfig {
    /// Default transition spec for all properties.
    pub default_spec: Option<TransitionSpec>,
    /// Per-property transition specs (override default).
    pub property_specs: HashMap<AnimatableProperty, TransitionSpec>,
}

impl Default for NodeTransitionConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeTransitionConfig {
    /// Create a new empty config.
    pub fn new() -> Self {
        Self {
            default_spec: None,
            property_specs: HashMap::new(),
        }
    }

    /// Create a config with a default "all" transition.
    pub fn all(spec: TransitionSpec) -> Self {
        Self {
            default_spec: Some(spec),
            property_specs: HashMap::new(),
        }
    }

    /// Set the default transition spec for all properties.
    pub fn with_default(mut self, spec: TransitionSpec) -> Self {
        self.default_spec = Some(spec);
        self
    }

    /// Set a specific transition spec for a property.
    pub fn with_property(mut self, property: AnimatableProperty, spec: TransitionSpec) -> Self {
        self.property_specs.insert(property, spec);
        self
    }

    /// Get the transition spec for a property.
    pub fn get_spec(&self, property: AnimatableProperty) -> Option<&TransitionSpec> {
        self.property_specs
            .get(&property)
            .or(self.default_spec.as_ref())
    }

    /// Check if this config has any transitions configured.
    pub fn has_transitions(&self) -> bool {
        self.default_spec.is_some() || !self.property_specs.is_empty()
    }
}

/// Interaction state for a node (hover, focus, active, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InteractionState {
    /// Mouse is hovering over the element.
    pub hovered: bool,
    /// Element has keyboard focus.
    pub focused: bool,
    /// Element is being pressed/activated.
    pub active: bool,
    /// Element is disabled.
    pub disabled: bool,
}

impl InteractionState {
    /// Create a new default interaction state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if any interaction state is active.
    pub fn is_interactive(&self) -> bool {
        self.hovered || self.focused || self.active
    }

    /// Set hover state.
    pub fn with_hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    /// Set focus state.
    pub fn with_focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Set active state.
    pub fn with_active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Set disabled state.
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Tracks property state changes for automatic transition triggering.
///
/// The `StateTracker` maintains snapshots of property values for nodes and
/// compares them to detect changes. When a change is detected and the node
/// has a transition configured for that property, a transition is automatically
/// started via the `AnimationManager`.
#[derive(Debug, Default)]
pub struct StateTracker {
    /// Last known property snapshots for each node.
    snapshots: HashMap<String, PropertySnapshot>,
    /// Transition configurations for each node.
    transition_configs: HashMap<String, NodeTransitionConfig>,
    /// Interaction states for each node.
    interaction_states: HashMap<String, InteractionState>,
}

impl StateTracker {
    /// Create a new state tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the transition configuration for a node.
    pub fn set_transition_config(&mut self, node_id: &str, config: NodeTransitionConfig) {
        self.transition_configs.insert(node_id.to_string(), config);
    }

    /// Set a simple "all properties" transition for a node.
    pub fn set_transition_spec(&mut self, node_id: &str, spec: TransitionSpec) {
        self.transition_configs
            .insert(node_id.to_string(), NodeTransitionConfig::all(spec));
    }

    /// Get the transition configuration for a node.
    pub fn get_transition_config(&self, node_id: &str) -> Option<&NodeTransitionConfig> {
        self.transition_configs.get(node_id)
    }

    /// Remove the transition configuration for a node.
    pub fn remove_transition_config(&mut self, node_id: &str) {
        self.transition_configs.remove(node_id);
    }

    /// Snapshot property values for a node.
    ///
    /// This stores the current property values so future changes can be detected.
    pub fn snapshot_properties(&mut self, node_id: &str, snapshot: PropertySnapshot) {
        self.snapshots.insert(node_id.to_string(), snapshot);
    }

    /// Get the last snapshot for a node.
    pub fn get_snapshot(&self, node_id: &str) -> Option<&PropertySnapshot> {
        self.snapshots.get(node_id)
    }

    /// Update a single property in the snapshot.
    pub fn update_snapshot_property(
        &mut self,
        node_id: &str,
        property: AnimatableProperty,
        value: AnimatableValue,
    ) {
        self.snapshots
            .entry(node_id.to_string())
            .or_default()
            .set(property, value);
    }

    /// Set the interaction state for a node.
    pub fn set_interaction_state(&mut self, node_id: &str, state: InteractionState) {
        self.interaction_states
            .insert(node_id.to_string(), state);
    }

    /// Get the interaction state for a node.
    pub fn get_interaction_state(&self, node_id: &str) -> InteractionState {
        self.interaction_states
            .get(node_id)
            .copied()
            .unwrap_or_default()
    }

    /// Update hover state for a node and return whether it changed.
    pub fn set_hovered(&mut self, node_id: &str, hovered: bool) -> bool {
        let state = self
            .interaction_states
            .entry(node_id.to_string())
            .or_default();
        let changed = state.hovered != hovered;
        state.hovered = hovered;
        changed
    }

    /// Update focus state for a node and return whether it changed.
    pub fn set_focused(&mut self, node_id: &str, focused: bool) -> bool {
        let state = self
            .interaction_states
            .entry(node_id.to_string())
            .or_default();
        let changed = state.focused != focused;
        state.focused = focused;
        changed
    }

    /// Update active state for a node and return whether it changed.
    pub fn set_active(&mut self, node_id: &str, active: bool) -> bool {
        let state = self
            .interaction_states
            .entry(node_id.to_string())
            .or_default();
        let changed = state.active != active;
        state.active = active;
        changed
    }

    /// Detect property changes and trigger transitions.
    ///
    /// Compares the new property values against the stored snapshot and starts
    /// transitions for any changed properties that have transition configurations.
    ///
    /// Returns the number of transitions started.
    pub fn detect_and_trigger_transitions(
        &mut self,
        node_id: &str,
        new_values: &PropertySnapshot,
        animation_manager: &mut AnimationManager,
    ) -> usize {
        let config = match self.transition_configs.get(node_id) {
            Some(c) if c.has_transitions() => c,
            _ => {
                // No transitions configured, just update snapshot
                self.snapshots
                    .insert(node_id.to_string(), new_values.clone());
                return 0;
            }
        };

        let old_snapshot = self.snapshots.get(node_id);
        let mut transitions_started = 0;

        for (property, new_value) in new_values.iter() {
            // Get old value (or skip if no previous snapshot)
            let old_value = old_snapshot.and_then(|s| s.get(*property));

            // Check if value changed
            let changed = match old_value {
                Some(old) => old != new_value,
                None => true, // No previous value, treat as changed
            };

            if changed {
                // Check if we have a transition spec for this property
                if let Some(spec) = config.get_spec(*property) {
                    // Get the from value (current animated value or old snapshot value)
                    let from_value = animation_manager
                        .get_animated_value(node_id, *property)
                        .or_else(|| old_value.cloned())
                        .unwrap_or_else(|| new_value.clone());

                    animation_manager.start_transition(
                        node_id,
                        *property,
                        from_value,
                        new_value.clone(),
                        spec,
                    );
                    transitions_started += 1;
                }
            }
        }

        // Update snapshot
        self.snapshots
            .insert(node_id.to_string(), new_values.clone());

        transitions_started
    }

    /// Trigger a transition for a specific property change.
    ///
    /// This is useful when you know a specific property has changed and want
    /// to trigger a transition without doing full snapshot comparison.
    pub fn trigger_transition(
        &mut self,
        node_id: &str,
        property: AnimatableProperty,
        from_value: AnimatableValue,
        to_value: AnimatableValue,
        animation_manager: &mut AnimationManager,
    ) -> bool {
        let spec = match self.transition_configs.get(node_id) {
            Some(config) => config.get_spec(property),
            None => None,
        };

        if let Some(spec) = spec {
            animation_manager.start_transition(
                node_id,
                property,
                from_value,
                to_value.clone(),
                spec,
            );

            // Update snapshot
            self.snapshots
                .entry(node_id.to_string())
                .or_default()
                .set(property, to_value);

            true
        } else {
            false
        }
    }

    /// Clear the snapshot for a node.
    pub fn clear_snapshot(&mut self, node_id: &str) {
        self.snapshots.remove(node_id);
    }

    /// Clear all snapshots.
    pub fn clear_all_snapshots(&mut self) {
        self.snapshots.clear();
    }

    /// Clear all state (snapshots, configs, interaction states).
    pub fn clear(&mut self) {
        self.snapshots.clear();
        self.transition_configs.clear();
        self.interaction_states.clear();
    }

    /// Get the number of tracked nodes.
    pub fn tracked_count(&self) -> usize {
        self.snapshots.len()
    }

    /// Get the number of nodes with transition configs.
    pub fn config_count(&self) -> usize {
        self.transition_configs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::easing::EasingFunction;

    #[test]
    fn test_property_snapshot() {
        let mut snapshot = PropertySnapshot::new();
        assert!(snapshot.is_empty());

        snapshot.set(AnimatableProperty::Width, AnimatableValue::F64 { value: 100.0 });
        snapshot.set(
            AnimatableProperty::BackgroundColor,
            AnimatableValue::Color {
                rgba: [1.0, 0.0, 0.0, 1.0],
            },
        );

        assert_eq!(snapshot.len(), 2);
        assert!(!snapshot.is_empty());

        let width = snapshot.get(AnimatableProperty::Width).unwrap();
        assert_eq!(width.as_f64(), Some(100.0));
    }

    #[test]
    fn test_node_transition_config() {
        let config = NodeTransitionConfig::all(TransitionSpec::all(300.0))
            .with_property(
                AnimatableProperty::Opacity,
                TransitionSpec::all(500.0).with_easing(EasingFunction::EaseOut),
            );

        // Should get specific spec for opacity
        let opacity_spec = config.get_spec(AnimatableProperty::Opacity).unwrap();
        assert_eq!(opacity_spec.duration_ms, 500.0);

        // Should get default spec for width
        let width_spec = config.get_spec(AnimatableProperty::Width).unwrap();
        assert_eq!(width_spec.duration_ms, 300.0);
    }

    #[test]
    fn test_interaction_state() {
        let state = InteractionState::new()
            .with_hovered(true)
            .with_focused(false);

        assert!(state.hovered);
        assert!(!state.focused);
        assert!(!state.active);
        assert!(state.is_interactive());

        let disabled = InteractionState::new().with_disabled(true);
        assert!(!disabled.is_interactive());
    }

    #[test]
    fn test_state_tracker_detect_changes() {
        let mut tracker = StateTracker::new();
        let mut manager = AnimationManager::new();

        // Configure transitions
        tracker.set_transition_spec(
            "node_1",
            TransitionSpec::all(300.0).with_easing(EasingFunction::Linear),
        );

        // Set initial snapshot
        let mut initial = PropertySnapshot::new();
        initial.set(AnimatableProperty::Width, AnimatableValue::F64 { value: 100.0 });
        initial.set(AnimatableProperty::Opacity, AnimatableValue::F64 { value: 1.0 });
        tracker.snapshot_properties("node_1", initial);

        // Create new values with changes
        let mut new_values = PropertySnapshot::new();
        new_values.set(AnimatableProperty::Width, AnimatableValue::F64 { value: 200.0 }); // Changed
        new_values.set(AnimatableProperty::Opacity, AnimatableValue::F64 { value: 1.0 }); // Same

        // Detect and trigger
        let transitions = tracker.detect_and_trigger_transitions("node_1", &new_values, &mut manager);

        // Should have started 1 transition (width changed, opacity same)
        assert_eq!(transitions, 1);
        assert_eq!(manager.active_count(), 1);

        // Check the transition was started for width
        let width_value = manager.get_animated_value("node_1", AnimatableProperty::Width);
        assert!(width_value.is_some());
    }

    #[test]
    fn test_state_tracker_no_config() {
        let mut tracker = StateTracker::new();
        let mut manager = AnimationManager::new();

        // No transition config set
        let mut new_values = PropertySnapshot::new();
        new_values.set(AnimatableProperty::Width, AnimatableValue::F64 { value: 200.0 });

        // Should not start any transitions
        let transitions = tracker.detect_and_trigger_transitions("node_1", &new_values, &mut manager);
        assert_eq!(transitions, 0);
        assert_eq!(manager.active_count(), 0);

        // But snapshot should still be updated
        assert!(tracker.get_snapshot("node_1").is_some());
    }

    #[test]
    fn test_state_tracker_trigger_transition() {
        let mut tracker = StateTracker::new();
        let mut manager = AnimationManager::new();

        tracker.set_transition_spec("node_1", TransitionSpec::all(300.0));

        let triggered = tracker.trigger_transition(
            "node_1",
            AnimatableProperty::Opacity,
            AnimatableValue::F64 { value: 1.0 },
            AnimatableValue::F64 { value: 0.0 },
            &mut manager,
        );

        assert!(triggered);
        assert_eq!(manager.active_count(), 1);

        // Snapshot should be updated
        let snapshot = tracker.get_snapshot("node_1").unwrap();
        let opacity = snapshot.get(AnimatableProperty::Opacity).unwrap();
        assert_eq!(opacity.as_f64(), Some(0.0));
    }

    #[test]
    fn test_state_tracker_hover_state() {
        let mut tracker = StateTracker::new();

        // Initial state should be not hovered
        let state = tracker.get_interaction_state("node_1");
        assert!(!state.hovered);

        // Set hovered
        let changed = tracker.set_hovered("node_1", true);
        assert!(changed);

        let state = tracker.get_interaction_state("node_1");
        assert!(state.hovered);

        // Setting same state should not report change
        let changed = tracker.set_hovered("node_1", true);
        assert!(!changed);

        // Setting different state should report change
        let changed = tracker.set_hovered("node_1", false);
        assert!(changed);
    }

    #[test]
    fn test_state_tracker_clear() {
        let mut tracker = StateTracker::new();

        tracker.set_transition_spec("node_1", TransitionSpec::all(300.0));
        tracker.snapshot_properties("node_1", PropertySnapshot::new());
        tracker.set_hovered("node_1", true);

        assert_eq!(tracker.tracked_count(), 1);
        assert_eq!(tracker.config_count(), 1);

        tracker.clear();

        assert_eq!(tracker.tracked_count(), 0);
        assert_eq!(tracker.config_count(), 0);
        assert!(!tracker.get_interaction_state("node_1").hovered);
    }
}
