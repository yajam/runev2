//! Animation events for lifecycle callbacks.
//!
//! This module provides event types and an event queue for tracking animation
//! lifecycle events (start, end, cancel, iteration). Events can be polled
//! after each animation update to respond to animation state changes.
//!
//! # Usage
//!
//! ```ignore
//! use rune_scene::animation::{AnimationManager, AnimationEvent, TransitionEvent};
//!
//! let mut manager = AnimationManager::new();
//!
//! // Start some animations...
//! manager.start_transition(...);
//!
//! // Update animations
//! manager.update(16.67);
//!
//! // Poll events
//! for event in manager.drain_events() {
//!     match event {
//!         AnimationEventKind::Transition(TransitionEvent::Ended { node_id, property }) => {
//!             println!("Transition ended for {} {:?}", node_id, property);
//!         }
//!         AnimationEventKind::Keyframe(AnimationEvent::Iteration { node_id, iteration, .. }) => {
//!             println!("Animation iteration {} for {}", iteration, node_id);
//!         }
//!         _ => {}
//!     }
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use super::types::{AnimatableProperty, AnimationId};

/// Event emitted when a keyframe animation changes state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnimationEvent {
    /// Animation has started playing.
    Started {
        /// The animation instance ID.
        animation_id: AnimationId,
        /// The node this animation is applied to.
        node_id: String,
        /// Name of the animation (from KeyframeAnimation).
        animation_name: String,
    },
    /// Animation has completed all iterations.
    Ended {
        /// The animation instance ID.
        animation_id: AnimationId,
        /// The node this animation was applied to.
        node_id: String,
        /// Name of the animation.
        animation_name: String,
    },
    /// Animation was cancelled before completion.
    Cancelled {
        /// The animation instance ID.
        animation_id: AnimationId,
        /// The node this animation was applied to.
        node_id: String,
        /// Name of the animation.
        animation_name: String,
    },
    /// Animation completed one iteration (for multi-iteration animations).
    Iteration {
        /// The animation instance ID.
        animation_id: AnimationId,
        /// The node this animation is applied to.
        node_id: String,
        /// Name of the animation.
        animation_name: String,
        /// The iteration that just completed (0-indexed).
        iteration: u32,
    },
}

impl AnimationEvent {
    /// Get the node ID for this event.
    pub fn node_id(&self) -> &str {
        match self {
            Self::Started { node_id, .. }
            | Self::Ended { node_id, .. }
            | Self::Cancelled { node_id, .. }
            | Self::Iteration { node_id, .. } => node_id,
        }
    }

    /// Get the animation ID for this event.
    pub fn animation_id(&self) -> AnimationId {
        match self {
            Self::Started { animation_id, .. }
            | Self::Ended { animation_id, .. }
            | Self::Cancelled { animation_id, .. }
            | Self::Iteration { animation_id, .. } => *animation_id,
        }
    }

    /// Get the animation name for this event.
    pub fn animation_name(&self) -> &str {
        match self {
            Self::Started { animation_name, .. }
            | Self::Ended { animation_name, .. }
            | Self::Cancelled { animation_name, .. }
            | Self::Iteration { animation_name, .. } => animation_name,
        }
    }
}

/// Event emitted when a transition changes state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransitionEvent {
    /// Transition has started.
    Started {
        /// The transition instance ID.
        transition_id: AnimationId,
        /// The node this transition is applied to.
        node_id: String,
        /// The property being transitioned.
        property: AnimatableProperty,
    },
    /// Transition has completed normally.
    Ended {
        /// The transition instance ID.
        transition_id: AnimationId,
        /// The node this transition was applied to.
        node_id: String,
        /// The property that was transitioned.
        property: AnimatableProperty,
    },
    /// Transition was cancelled (e.g., by starting a new transition).
    Cancelled {
        /// The transition instance ID.
        transition_id: AnimationId,
        /// The node this transition was applied to.
        node_id: String,
        /// The property that was being transitioned.
        property: AnimatableProperty,
    },
}

impl TransitionEvent {
    /// Get the node ID for this event.
    pub fn node_id(&self) -> &str {
        match self {
            Self::Started { node_id, .. }
            | Self::Ended { node_id, .. }
            | Self::Cancelled { node_id, .. } => node_id,
        }
    }

    /// Get the transition ID for this event.
    pub fn transition_id(&self) -> AnimationId {
        match self {
            Self::Started { transition_id, .. }
            | Self::Ended { transition_id, .. }
            | Self::Cancelled { transition_id, .. } => *transition_id,
        }
    }

    /// Get the property for this event.
    pub fn property(&self) -> AnimatableProperty {
        match self {
            Self::Started { property, .. }
            | Self::Ended { property, .. }
            | Self::Cancelled { property, .. } => *property,
        }
    }
}

/// Wrapper enum for both animation and transition events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnimationEventKind {
    /// A keyframe animation event.
    Keyframe(AnimationEvent),
    /// A transition event.
    Transition(TransitionEvent),
}

impl AnimationEventKind {
    /// Get the node ID for this event.
    pub fn node_id(&self) -> &str {
        match self {
            Self::Keyframe(e) => e.node_id(),
            Self::Transition(e) => e.node_id(),
        }
    }

    /// Check if this is a "started" event.
    pub fn is_started(&self) -> bool {
        matches!(
            self,
            Self::Keyframe(AnimationEvent::Started { .. })
                | Self::Transition(TransitionEvent::Started { .. })
        )
    }

    /// Check if this is an "ended" event.
    pub fn is_ended(&self) -> bool {
        matches!(
            self,
            Self::Keyframe(AnimationEvent::Ended { .. })
                | Self::Transition(TransitionEvent::Ended { .. })
        )
    }

    /// Check if this is a "cancelled" event.
    pub fn is_cancelled(&self) -> bool {
        matches!(
            self,
            Self::Keyframe(AnimationEvent::Cancelled { .. })
                | Self::Transition(TransitionEvent::Cancelled { .. })
        )
    }
}

impl From<AnimationEvent> for AnimationEventKind {
    fn from(event: AnimationEvent) -> Self {
        Self::Keyframe(event)
    }
}

impl From<TransitionEvent> for AnimationEventKind {
    fn from(event: TransitionEvent) -> Self {
        Self::Transition(event)
    }
}

/// Queue for collecting animation events during update cycles.
#[derive(Debug, Default)]
pub struct EventQueue {
    events: VecDeque<AnimationEventKind>,
}

impl EventQueue {
    /// Create a new empty event queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push an animation event onto the queue.
    pub fn push_animation_event(&mut self, event: AnimationEvent) {
        self.events.push_back(AnimationEventKind::Keyframe(event));
    }

    /// Push a transition event onto the queue.
    pub fn push_transition_event(&mut self, event: TransitionEvent) {
        self.events.push_back(AnimationEventKind::Transition(event));
    }

    /// Push any event kind onto the queue.
    pub fn push(&mut self, event: AnimationEventKind) {
        self.events.push_back(event);
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Get the number of pending events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Pop the next event from the queue.
    pub fn pop(&mut self) -> Option<AnimationEventKind> {
        self.events.pop_front()
    }

    /// Drain all events from the queue, returning an iterator.
    pub fn drain(&mut self) -> impl Iterator<Item = AnimationEventKind> + '_ {
        self.events.drain(..)
    }

    /// Peek at the next event without removing it.
    pub fn peek(&self) -> Option<&AnimationEventKind> {
        self.events.front()
    }

    /// Clear all pending events.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Get events for a specific node.
    pub fn events_for_node(&self, node_id: &str) -> Vec<&AnimationEventKind> {
        self.events
            .iter()
            .filter(|e| e.node_id() == node_id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_event_accessors() {
        let event = AnimationEvent::Started {
            animation_id: AnimationId(1),
            node_id: "node_1".to_string(),
            animation_name: "fade".to_string(),
        };

        assert_eq!(event.node_id(), "node_1");
        assert_eq!(event.animation_id(), AnimationId(1));
        assert_eq!(event.animation_name(), "fade");
    }

    #[test]
    fn test_transition_event_accessors() {
        let event = TransitionEvent::Ended {
            transition_id: AnimationId(2),
            node_id: "node_2".to_string(),
            property: AnimatableProperty::Opacity,
        };

        assert_eq!(event.node_id(), "node_2");
        assert_eq!(event.transition_id(), AnimationId(2));
        assert_eq!(event.property(), AnimatableProperty::Opacity);
    }

    #[test]
    fn test_event_kind_predicates() {
        let started = AnimationEventKind::Keyframe(AnimationEvent::Started {
            animation_id: AnimationId(1),
            node_id: "n".to_string(),
            animation_name: "a".to_string(),
        });
        assert!(started.is_started());
        assert!(!started.is_ended());
        assert!(!started.is_cancelled());

        let ended = AnimationEventKind::Transition(TransitionEvent::Ended {
            transition_id: AnimationId(1),
            node_id: "n".to_string(),
            property: AnimatableProperty::Width,
        });
        assert!(!ended.is_started());
        assert!(ended.is_ended());
        assert!(!ended.is_cancelled());

        let cancelled = AnimationEventKind::Keyframe(AnimationEvent::Cancelled {
            animation_id: AnimationId(1),
            node_id: "n".to_string(),
            animation_name: "a".to_string(),
        });
        assert!(!cancelled.is_started());
        assert!(!cancelled.is_ended());
        assert!(cancelled.is_cancelled());
    }

    #[test]
    fn test_event_queue_operations() {
        let mut queue = EventQueue::new();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);

        queue.push_animation_event(AnimationEvent::Started {
            animation_id: AnimationId(1),
            node_id: "node_1".to_string(),
            animation_name: "fade".to_string(),
        });

        queue.push_transition_event(TransitionEvent::Started {
            transition_id: AnimationId(2),
            node_id: "node_2".to_string(),
            property: AnimatableProperty::Opacity,
        });

        assert!(!queue.is_empty());
        assert_eq!(queue.len(), 2);

        // Pop first event
        let event = queue.pop().unwrap();
        assert!(matches!(event, AnimationEventKind::Keyframe(AnimationEvent::Started { .. })));
        assert_eq!(queue.len(), 1);

        // Pop second event
        let event = queue.pop().unwrap();
        assert!(matches!(event, AnimationEventKind::Transition(TransitionEvent::Started { .. })));
        assert_eq!(queue.len(), 0);

        // Queue is now empty
        assert!(queue.pop().is_none());
    }

    #[test]
    fn test_event_queue_drain() {
        let mut queue = EventQueue::new();

        queue.push_animation_event(AnimationEvent::Ended {
            animation_id: AnimationId(1),
            node_id: "n".to_string(),
            animation_name: "a".to_string(),
        });
        queue.push_transition_event(TransitionEvent::Ended {
            transition_id: AnimationId(2),
            node_id: "n".to_string(),
            property: AnimatableProperty::Width,
        });

        let events: Vec<_> = queue.drain().collect();
        assert_eq!(events.len(), 2);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_event_queue_events_for_node() {
        let mut queue = EventQueue::new();

        queue.push_animation_event(AnimationEvent::Started {
            animation_id: AnimationId(1),
            node_id: "node_1".to_string(),
            animation_name: "a".to_string(),
        });
        queue.push_animation_event(AnimationEvent::Started {
            animation_id: AnimationId(2),
            node_id: "node_2".to_string(),
            animation_name: "b".to_string(),
        });
        queue.push_transition_event(TransitionEvent::Started {
            transition_id: AnimationId(3),
            node_id: "node_1".to_string(),
            property: AnimatableProperty::Opacity,
        });

        let node_1_events = queue.events_for_node("node_1");
        assert_eq!(node_1_events.len(), 2);

        let node_2_events = queue.events_for_node("node_2");
        assert_eq!(node_2_events.len(), 1);

        let node_3_events = queue.events_for_node("node_3");
        assert_eq!(node_3_events.len(), 0);
    }

    #[test]
    fn test_event_serialization() {
        let event = AnimationEvent::Iteration {
            animation_id: AnimationId(42),
            node_id: "button_1".to_string(),
            animation_name: "pulse".to_string(),
            iteration: 3,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("iteration"));
        assert!(json.contains("button_1"));
        assert!(json.contains("pulse"));

        let parsed: AnimationEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }
}
