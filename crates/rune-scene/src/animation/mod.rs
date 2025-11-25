//! Animation module for CSS-like animations in the rune-scene IR runtime.
//!
//! This module provides:
//! - **Transitions**: Smooth interpolation between property values on state change
//! - **Keyframe Animations**: Multi-step animations with defined keyframes
//! - **Easing Functions**: Standard CSS timing functions
//! - **Animation Events**: Callbacks for animation lifecycle
//!
//! # Architecture
//!
//! ```text
//! AnimationManager
//!   ├── Active Transitions (property → value interpolation)
//!   └── Active Keyframe Animations (multi-keyframe sequences)
//!
//! AnimatedPropertyResolver
//!   └── Queries manager for current animated values during rendering
//! ```

pub mod easing;
pub mod events;
pub mod interpolate;
pub mod keyframes;
pub mod manager;
pub mod resolver;
pub mod schema;
pub mod state_tracker;
pub mod transform;
pub mod transition;
pub mod types;

pub use easing::{EasingFunction, StepPosition};
pub use events::{AnimationEvent, AnimationEventKind, EventQueue, TransitionEvent};
pub use interpolate::Interpolate;
pub use keyframes::{
    ActiveKeyframeAnimation, AnimationDirection, AnimationFillMode, AnimationPlayState,
    IterationCount, Keyframe, KeyframeAnimation,
};
pub use manager::AnimationManager;
pub use resolver::AnimatedPropertyResolver;
pub use schema::{AnimationRef, NodeAnimationSpec};
pub use state_tracker::{InteractionState, NodeTransitionConfig, PropertySnapshot, StateTracker};
pub use transition::{ActiveTransition, TransitionGroup, TransitionSpec, TransitionTarget};
pub use transform::{DecomposedTransform, NamedOrigin, Transform2D, TransformOrigin, TransformStack};
pub use types::{
    AnimatableBoxShadow, AnimatableEdgeInsets, AnimatableProperty, AnimatableTransform,
    AnimatableValue, AnimationId, AnimationState, Visibility,
};
