# Animation Module Implementation Checklist

This document outlines the implementation plan for adding CSS-like animations to the rune-scene IR runtime.

## Overview

The animation module will provide CSS-like animation capabilities for IR nodes, supporting:
- **Transitions**: Smooth interpolation between property values on state change
- **Keyframe Animations**: Multi-step animations with defined keyframes
- **Easing Functions**: Standard CSS timing functions
- **Animation Events**: Callbacks for animation lifecycle

---

## Phase 1: Core Animation Infrastructure ✅ COMPLETED

### 1.1 Animation Types & Data Structures

- [x] Create `crates/rune-scene/src/animation/mod.rs` module entry point
- [x] Define `AnimatableValue` enum for all animatable property types:
  - [x] `F64(f64)` - for numeric properties (width, height, opacity, etc.)
  - [x] `Color([f32; 4])` - for RGBA color values
  - [x] `EdgeInsets { top, right, bottom, left }` - for padding/margin
  - [x] `Transform { translate_x, translate_y, scale_x, scale_y, rotate }` - for transforms
  - [x] `BoxShadow { offset_x, offset_y, blur, color }` - for shadows
- [x] Define `AnimatableProperty` enum mapping to IR node properties:
  - [x] Geometry: `Width`, `Height`, `MinWidth`, `MinHeight`, `MaxWidth`, `MaxHeight`
  - [x] Spacing: `PaddingTop`, `PaddingRight`, `PaddingBottom`, `PaddingLeft`, `MarginTop`, `MarginRight`, `MarginBottom`, `MarginLeft`
  - [x] Visual: `Opacity`, `CornerRadius`, `BorderWidth`, `BorderColor`
  - [x] Background: `BackgroundColor`
  - [x] Text: `FontSize`, `TextColor`
  - [x] Transform: `TranslateX`, `TranslateY`, `ScaleX`, `ScaleY`, `Rotate`
  - [x] Shadow: `BoxShadowOffsetX`, `BoxShadowOffsetY`, `BoxShadowBlur`, `BoxShadowColor`
- [x] Create `AnimationId` type (u64 or UUID)
- [x] Create `AnimationState` enum: `Pending`, `Running`, `Paused`, `Finished`, `Cancelled`

### 1.2 Easing Functions

- [x] Create `crates/rune-scene/src/animation/easing.rs`
- [x] Implement `EasingFunction` enum:
  - [x] `Linear`
  - [x] `Ease` (default CSS ease)
  - [x] `EaseIn`
  - [x] `EaseOut`
  - [x] `EaseInOut`
  - [x] `CubicBezier(f32, f32, f32, f32)` - custom bezier curves
  - [x] `Steps(u32, StepPosition)` - stepped animations
  - [ ] `Spring { stiffness, damping, mass }` - physics-based (stretch goal)
- [x] Implement `StepPosition` enum: `Start`, `End`, `Both`, `None`
- [x] Implement `fn evaluate(&self, t: f32) -> f32` for each easing function
- [x] Add unit tests for all easing functions

### 1.3 Interpolation System

- [x] Create `crates/rune-scene/src/animation/interpolate.rs`
- [x] Implement `Interpolate` trait:
  ```rust
  pub trait Interpolate {
      fn interpolate(&self, to: &Self, t: f32) -> Self;
  }
  ```
- [x] Implement `Interpolate` for:
  - [x] `f64` (linear interpolation)
  - [x] `[f32; 4]` (color - consider color space for smooth blending)
  - [x] `EdgeInsets` (per-component interpolation)
  - [x] `Transform` (decomposed interpolation)
  - [x] `BoxShadowSpec` (per-component interpolation)
- [x] Add color space handling (sRGB vs linear for smooth gradients)
- [x] Add unit tests for interpolation

---

## Phase 2: Transition System ✅ COMPLETED

### 2.1 Transition Definition

- [x] Create `crates/rune-scene/src/animation/transition.rs`
- [x] Define `TransitionSpec` struct:
  ```rust
  pub struct TransitionSpec {
      pub target: TransitionTarget,    // Property or All
      pub duration_ms: f32,
      pub delay_ms: f32,
      pub easing: EasingFunction,
  }
  ```
- [x] Define `TransitionGroup` for multiple property transitions
- [ ] Add `transition` field to relevant IR spec structs (FlexContainerSpec, etc.) - *deferred to Phase 4*
- [ ] Support shorthand `transition: all 300ms ease` style definitions - *deferred to Phase 4*

### 2.2 Active Transition Tracking

- [x] Create `ActiveTransition` struct:
  ```rust
  pub struct ActiveTransition {
      pub id: AnimationId,
      pub node_id: String,
      pub property: AnimatableProperty,
      pub from_value: AnimatableValue,
      pub to_value: AnimatableValue,
      pub duration_ms: f32,
      pub delay_ms: f32,
      pub elapsed_ms: f32,
      pub easing: EasingFunction,
      pub state: AnimationState,
  }
  ```
- [x] Implement `fn current_value(&self) -> AnimatableValue`
- [x] Implement `fn update(&mut self, delta_ms: f32) -> bool` (returns true if still running)
- [x] Implement `fn retarget(&mut self, new_to_value, spec)` for transition interruption
- [x] Implement `fn pause()`, `fn resume()`, `fn cancel()`

### 2.3 Transition Manager

- [x] Create `AnimationManager` struct in `crates/rune-scene/src/animation/manager.rs`
- [x] Implement `fn start_transition(node_id, property, from, to, spec) -> AnimationId`
- [x] Implement `fn update(&mut self, delta_ms: f32)` - advance all active transitions
- [x] Implement `fn get_animated_value(node_id, property) -> Option<AnimatableValue>`
- [x] Implement `fn get_all_animated_values(node_id) -> HashMap<Property, Value>`
- [x] Implement `fn cancel_transition(id)` and `fn cancel_all_for_node(node_id)`
- [x] Handle transition interruption (new transition starts while one is running)
- [x] Implement pause/resume for individual transitions and all transitions on a node
- [x] Implement `has_active_animations()`, `needs_redraw()`, `active_count()`
- [x] Ensure `AnimationManager` is `Send` for async compatibility

---

## Phase 3: Keyframe Animation System ✅ COMPLETED

### 3.1 Keyframe Definition

- [x] Create `crates/rune-scene/src/animation/keyframes.rs`
- [x] Define `Keyframe` struct:
  ```rust
  pub struct Keyframe {
      pub offset: f32,  // 0.0 to 1.0 (0% to 100%)
      pub values: HashMap<AnimatableProperty, AnimatableValue>,
      pub easing: Option<EasingFunction>,  // easing TO this keyframe
  }
  ```
- [x] Define `KeyframeAnimation` struct with builder pattern
- [x] Define `IterationCount` enum: `Count(f32)`, `Infinite`
- [x] Define `AnimationDirection`: `Normal`, `Reverse`, `Alternate`, `AlternateReverse`
- [x] Define `AnimationFillMode`: `None`, `Forwards`, `Backwards`, `Both`
- [x] Define `AnimationPlayState`: `Running`, `Paused`

### 3.2 Animation Registry

- [x] Create animation registry in `AnimationManager` for named keyframe animations
- [x] Implement `register_animation()` and `get_registered_animation()`
- [x] Implement `start_registered_animation()` to start by name
- [ ] Support defining animations in IR schema (ViewDocument level) - *deferred to Phase 4*
- [ ] Support referencing animations by name from node specs - *deferred to Phase 4*

### 3.3 Active Animation Tracking

- [x] Create `ActiveKeyframeAnimation` struct with full lifecycle
- [x] Implement `find_keyframes()` to locate surrounding keyframes
- [x] Implement `value_at()` for keyframe interpolation
- [x] Implement `current_offset()` with direction handling
- [x] Implement `current_value()` and `current_values()`
- [x] Handle iteration counting and direction changes
- [x] Implement fill mode behavior (backwards/forwards)
- [x] Implement `pause()`, `resume()`, `cancel()`

### 3.4 Animation Manager Extension

- [x] Extend `AnimationManager` with keyframe animation storage and indexing
- [x] Implement `start_keyframe_animation(node_id, animation) -> AnimationId`
- [x] Implement `start_registered_animation(node_id, name) -> Option<AnimationId>`
- [x] Implement `pause_keyframe_animation(id)` and `resume_keyframe_animation(id)`
- [x] Implement `cancel_keyframe_animation(id)` and `cancel_keyframe_animations_for_node()`
- [x] Update `update()` to process both transitions and keyframe animations
- [x] Update `get_animated_value()` to check both (transitions take priority)
- [x] Update `get_all_animated_values()` to combine both sources
- [x] Add `keyframe_animation_count()` and `get_keyframe_animation()`
- [x] Update `has_active_animations()`, `active_count()`, `needs_redraw()`
- [x] Add `clear()` (preserves registry) and `clear_all()` (clears registry)

---

## Phase 4: IR Schema Integration ✅ COMPLETED

### 4.1 Spec Extensions

- [x] Add `animations` field to `ViewDocument`:
  ```rust
  pub struct ViewDocument {
      // ... existing fields
      pub animations: HashMap<String, KeyframeAnimationSpec>,
  }
  ```
- [x] Add animation properties to container specs (FlexContainerSpec, GridContainerSpec):
  ```rust
  pub struct FlexContainerSpec {
      // ... existing fields
      pub transition: Option<TransitionGroupSpec>,
      pub animation: Option<AnimationRefSpec>,
  }
  ```
- [x] Define `AnimationRefSpec` struct in `rune-ir/src/view/animation.rs`:
  ```rust
  pub struct AnimationRefSpec {
      pub name: String,
      pub duration_ms: Option<f32>,   // override
      pub delay_ms: Option<f32>,      // override
      pub iteration_count: Option<IterationCountSpec>,
      pub direction: Option<AnimationDirectionSpec>,
      pub fill_mode: Option<AnimationFillModeSpec>,
      pub autoplay: bool,
  }
  ```

### 4.2 Serde Support

- [x] Implement Serialize/Deserialize for all animation types in rune-scene
- [x] Create IR schema types in `rune-ir/src/view/animation.rs` for ViewDocument serialization
- [ ] Support CSS-like shorthand syntax parsing where appropriate - *deferred*
- [ ] Add JSON schema documentation for animation properties - *deferred*

### 4.3 Runtime Animation Types (rune-scene)

- [x] Add `AnimationRef` struct in `rune-scene/src/animation/schema.rs`
- [x] Add `NodeAnimationSpec` for combining transitions and animations
- [x] Export new types from animation module

---

## Phase 5: Rendering Integration ✅ COMPLETED (Infrastructure)

### 5.1 Animated Property Resolution

- [x] Create `crates/rune-scene/src/animation/resolver.rs`
- [x] Implement `AnimatedPropertyResolver`:
  ```rust
  pub struct AnimatedPropertyResolver<'a> {
      pub animation_manager: &'a AnimationManager,
  }

  impl AnimatedPropertyResolver {
      pub fn resolve_f64(&self, node_id: &ViewNodeId, property: AnimatableProperty, base: f64) -> f64;
      pub fn resolve_color(&self, node_id: &ViewNodeId, property: AnimatableProperty, base: [f32; 4]) -> [f32; 4];
      // ... etc
  }
  ```
- [x] Integrate AnimationManager into `IrRenderer` state
- [x] Add accessor methods: `animation_manager()`, `animation_manager_mut()`, `update_animations()`, `has_active_animations()`

### 5.2 Layout Integration

- [ ] Handle animated layout properties (width, height, padding, margin) - *deferred to element integration*
- [ ] Determine when to trigger Taffy relayout during animations - *deferred to element integration*
- [ ] Optimize: batch layout updates, skip when only visual properties change - *deferred to element integration*

### 5.3 Rendering Loop Integration

- [x] Add animation update to frame loop in `runner.rs`:
  ```rust
  // In RedrawRequested handler:
  let delta_ms = delta_time * 1000.0;
  let has_active_animations = ir_renderer.update_animations(delta_ms);
  ```
- [x] Request continuous redraw when animations are active
- [ ] Integrate animated values into element rendering functions - *deferred to element integration*

---

## Phase 6: State Change Detection ✅ COMPLETED

### 6.1 Property Change Tracking

- [x] Implement `PropertySnapshot` for tracking node property values
- [x] Implement `StateTracker` to detect property changes between frames
- [x] Implement `detect_and_trigger_transitions()` for auto-starting transitions
- [x] Implement `trigger_transition()` for explicit property change triggers

### 6.2 Trigger Mechanisms

- [x] Support explicit animation triggers via `StateTracker::trigger_transition()`
- [x] Implement `InteractionState` for hover/focus/active state tracking
- [x] Add `set_hovered()`, `set_focused()`, `set_active()` methods for state changes
- [x] Integrate `StateTracker` into `IrRenderer` with accessor methods
- [ ] Wire up hover/focus state changes in event handlers - *deferred to integration*

---

## Phase 7: Animation Events & Callbacks ✅ COMPLETED

### 7.1 Event Types

- [x] Define `AnimationEvent` enum:
  - [x] `Started { animation_id, node_id, animation_name }`
  - [x] `Ended { animation_id, node_id, animation_name }`
  - [x] `Cancelled { animation_id, node_id, animation_name }`
  - [x] `Iteration { animation_id, node_id, animation_name, iteration }`
- [x] Define `TransitionEvent` enum:
  - [x] `Started { transition_id, node_id, property }`
  - [x] `Ended { transition_id, node_id, property }`
  - [x] `Cancelled { transition_id, node_id, property }`
- [x] Define `AnimationEventKind` wrapper enum for both event types
- [x] Define `EventQueue` for collecting events during update cycles

### 7.2 Event Dispatch

- [x] Add event queue to animation manager
- [x] Implement event emission in `start_transition()`, `start_keyframe_animation()`, and `update()`
- [x] Implement event polling methods:
  - [x] `drain_events()` - drain all events from queue
  - [x] `has_pending_events()` - check if events pending
  - [x] `pending_event_count()` - get number of pending events
  - [x] `peek_event()` - peek at next event without removing
  - [x] `pop_event()` - pop single event from queue
  - [x] `events_for_node()` - get events for specific node
  - [x] `clear_events()` - clear all pending events
- [ ] Integrate with IR intent system for animation-triggered actions - *deferred to integration*

---

## Phase 8: Transform Support ✅ COMPLETED

### 8.1 Transform Stack

- [x] Create `Transform2D` struct with matrix operations
  - [x] 2D affine transformation matrix (3x2 layout)
  - [x] Matrix composition (`then`, `pre_multiply`)
  - [x] Matrix inversion
  - [x] Point and vector transformation
  - [x] Matrix format conversion (3x3, 4x4)
- [x] Create `DecomposedTransform` struct with individual components
  - [x] Translation (x, y)
  - [x] Rotation (radians)
  - [x] Scale (x, y)
  - [x] Skew (x, y)
- [x] Implement transform composition and decomposition
  - [x] `from_decomposed` - build matrix from components
  - [x] `decompose` - extract components from matrix
  - [x] `from_animatable` / `to_animatable` conversions
  - [x] Interpolation for `DecomposedTransform`
- [x] Add `transform` and `opacity` properties to IR specs
  - [x] Created `TransformSpec` in `rune-ir::view::animation`
  - [x] Created `TransformOriginSpec` and `NamedOriginSpec`
  - [x] Added `transform: Option<TransformSpec>` to `FlexContainerSpec` and `GridContainerSpec`
  - [x] Added `opacity: Option<f64>` to container specs

### 8.2 Transform Rendering

- [x] Create `TransformOrigin` enum with named and percentage origins
- [x] Implement `with_origin` for applying transforms relative to a point
- [x] Add `TransformStack` for composing multiple transforms
- [x] Integrate transform into resolver
  - [x] `resolve_transform_matrix` - resolve full transform with origin
  - [x] `resolve_opacity` - resolve opacity with clamping
  - [x] Support for both grouped Transform and individual component animations
- [ ] Apply transforms during display list building - *deferred to rendering integration*

---

## Phase 9: Opacity & Visibility ✅ COMPLETED

### 9.1 Opacity Support

- [x] Add `opacity: Option<f64>` to container specs
  - [x] Added to `FlexContainerSpec` and `GridContainerSpec`
  - [x] Opacity property already in `AnimatableProperty` enum
  - [x] `resolve_opacity()` method in resolver with clamping to [0.0, 1.0]
- [ ] Implement opacity in rendering (multiply alpha) - *deferred to rendering integration*
- [ ] Handle opacity inheritance for nested elements - *deferred to rendering integration*
  - Note: Opacity should be cumulative for nested elements (child opacity = parent opacity × child opacity)
  - The rendering layer will need to track the accumulated opacity through the scene graph

### 9.2 Visibility Transitions

- [x] Support `visibility` property with transition
  - [x] Created `Visibility` enum (Visible, Hidden, Collapsed)
  - [x] Added `Visibility` to `AnimatableProperty` and `AnimatableValue`
  - [x] Created `VisibilitySpec` enum in IR schema
  - [x] Added `visibility: Option<VisibilitySpec>` to container specs
  - [x] Added `resolve_visibility()` method in resolver
  - [x] Marked `Visibility` as layout-affecting (for Collapsed state)
- [ ] Handle rendering for different visibility states - *deferred to rendering integration*
  - `Visible`: Normal rendering
  - `Hidden`: Skip rendering but maintain layout space (CSS visibility: hidden)
  - `Collapsed`: Skip rendering and remove from layout (CSS display: none)

Note: Visibility transitions are typically discrete (stepped) rather than smooth. The animation
system handles this naturally - visibility will change at the specified point in the transition.

---

## Phase 10: Testing & Demo

### 10.1 Unit Tests

- [ ] Test all easing functions
- [ ] Test interpolation for all animatable types
- [ ] Test transition lifecycle (start, update, complete, cancel)
- [ ] Test keyframe animation with multiple iterations
- [ ] Test animation direction and fill modes

### 10.2 Integration Tests

- [ ] Test animation with layout changes
- [ ] Test multiple simultaneous animations on same node
- [ ] Test animation interruption scenarios
- [ ] Test performance with many active animations

### 10.3 Demo Scene

- [ ] Create `animation` demo scene in demo-app
- [ ] Showcase transitions on hover/focus
- [ ] Showcase keyframe animations
- [ ] Showcase various easing functions
- [ ] Add to `cargo run -p demo-app -- --scene=animation`

---

## Phase 11: Documentation

- [ ] Document animation module architecture in `docs/animation.md`
- [ ] Add animation examples to IR schema documentation
- [ ] Document performance considerations
- [ ] Add migration guide for existing IR documents
- [ ] Update `CLAUDE.md` with animation-related information

---

## Phase 12: Performance Optimization

### 12.1 Efficiency

- [ ] Batch animation updates
- [ ] Skip rendering unchanged frames
- [ ] Use dirty flags for animated properties
- [ ] Consider GPU-accelerated animations for transforms/opacity

### 12.2 Memory

- [ ] Pool animation objects
- [ ] Clean up completed animations promptly
- [ ] Limit maximum concurrent animations per node

---

## File Structure

```
crates/rune-scene/src/animation/
├── mod.rs              # Module exports ✅
├── types.rs            # AnimatableValue, AnimatableProperty, AnimationState ✅
├── easing.rs           # EasingFunction implementations ✅
├── interpolate.rs      # Interpolate trait and implementations ✅
├── transition.rs       # TransitionSpec, ActiveTransition ✅
├── keyframes.rs        # Keyframe, KeyframeAnimation ✅
├── manager.rs          # AnimationManager (transitions + keyframes) ✅
├── resolver.rs         # AnimatedPropertyResolver for rendering ✅
├── schema.rs           # AnimationRef, NodeAnimationSpec for runtime ✅
├── state_tracker.rs    # StateTracker, PropertySnapshot, InteractionState ✅
├── events.rs           # AnimationEvent, TransitionEvent, EventQueue ✅
└── transform.rs        # Transform2D, DecomposedTransform, TransformOrigin ✅

crates/rune-ir/src/view/
├── animation.rs        # IR schema types (EasingSpec, KeyframeSpec, etc.) ✅
└── ...
```

---

## Dependencies

- No new external crates required for core functionality
- Consider `interpolation` crate for additional easing curves (optional)
- Consider `palette` crate for better color interpolation (optional)

---

## Priority Order

1. **Core Infrastructure** (Phase 1) - Foundation for all animation work
2. **Transitions** (Phase 2) - Most common use case, immediate value
3. **Rendering Integration** (Phase 5) - Make transitions visible
4. **State Change Detection** (Phase 6) - Automatic transition triggering
5. **Keyframe Animations** (Phase 3-4) - Extended animation capabilities
6. **Transform Support** (Phase 8) - Enable transform animations
7. **Opacity** (Phase 9) - Common animation property
8. **Events** (Phase 7) - Animation lifecycle hooks
9. **Testing & Demo** (Phase 10) - Validation and showcase
10. **Documentation** (Phase 11) - Knowledge sharing
11. **Optimization** (Phase 12) - Performance tuning

---

## CSS Animation Parity Reference

This implementation aims for parity with these CSS features:
- `transition` property
- `@keyframes` rule
- `animation` property
- `animation-timing-function`
- `animation-duration`, `animation-delay`
- `animation-iteration-count`
- `animation-direction`
- `animation-fill-mode`
- `animation-play-state`
- `transform` property (2D subset)
- `opacity` property

---

## REMAINING WORK - Implementation Checklist

This section outlines all remaining work items for a complete animation system implementation.

### High Priority - Core Functionality

#### Element-Specific Property Animations

**Text Element Animations** (not yet implemented)
- [ ] Integrate animated `FontSize` into `render_text_element()`
  - Modify signature to accept `AnimatedPropertyResolver`
  - Resolve `AnimatableProperty::FontSize` before rendering
  - Update all call sites in `core.rs`
- [ ] Integrate animated `TextColor` into text rendering
  - Resolve `AnimatableProperty::TextColor` for text runs
  - Apply to ColorLinPremul before draw_text_run
- [ ] Support animated line height (if added to spec)

**Border Property Animations** (not yet implemented)
- [ ] Integrate animated `BorderWidth` into container rendering
  - Add border rendering support to `render_container_element()`
  - Resolve animated border width values
- [ ] Integrate animated `BorderColor` into container rendering
  - Resolve animated border color values
  - Apply to stroke brush
- [ ] Integrate animated `CornerRadius` into container rendering
  - Resolve animated corner radius values
  - Update rounded rectangle rendering

**Button Element Animations** (not yet implemented)
- [ ] Add resolver parameter to button element rendering
- [ ] Resolve animated background colors for button states
- [ ] Resolve animated text colors for button labels

**Other Element Types** (not yet implemented)
- [ ] Input elements (InputBox, TextArea)
  - Animated border colors
  - Animated background colors
  - Animated text colors
- [ ] Image elements
  - Animated opacity (if separate from container)
  - Animated dimensions (if different from layout)
- [ ] Link elements
  - Animated text color
  - Animated underline color

#### Canvas Opacity Layer Support

**Opacity Rendering** (infrastructure ready, rendering not implemented)
- [ ] Implement `Canvas::push_opacity()` method in rune-surface
  - Create opacity layer via offscreen texture or blend mode
  - Track opacity stack similar to transform stack
- [ ] Implement `Canvas::pop_opacity()` method
- [ ] Update `render_view_node_with_elements()` to use push/pop opacity
  - Currently resolves opacity but doesn't apply it (line 1200-1203 in core.rs)
  - Replace TODO comment with actual opacity layer push/pop

**Opacity Inheritance** (not yet implemented)
- [ ] Track accumulated opacity through scene graph traversal
- [ ] Multiply child opacity by parent opacity during rendering
- [ ] Handle opacity correctly for nested animated elements

#### Hover/Focus State Integration

**Event Handler Wiring** (infrastructure ready, not wired up)
- [ ] Wire up `set_node_hovered()` in mouse move handler
  - Location: `runner.rs` WindowEvent::CursorMoved
  - Check hit-testing results and update hover state
- [ ] Wire up `set_node_focused()` in click handler
  - Location: `runner.rs` WindowEvent::MouseInput
  - Update focus state on click
- [ ] Wire up `set_node_active()` for mouse down/up
  - Track active (pressed) state
- [ ] Test automatic transition triggers on state changes

### Medium Priority - Polish & Features

#### Animation Testing

**Unit Tests** (not yet implemented)
- [ ] Test all easing functions with known inputs/outputs
- [ ] Test interpolation for all AnimatableValue types
- [ ] Test transition lifecycle (start, update, complete, cancel)
- [ ] Test keyframe animation with multiple iterations
- [ ] Test animation direction (Normal, Reverse, Alternate, AlternateReverse)
- [ ] Test fill modes (None, Forwards, Backwards, Both)
- [ ] Test animation interruption and retargeting
- [ ] Test event queue (Started, Ended, Cancelled, Iteration)

**Integration Tests** (not yet implemented)
- [ ] Test layout animations triggering Taffy relayout
  - Verify children reflow correctly
  - Test padding/margin animations
- [ ] Test multiple simultaneous animations on same node
  - Different properties
  - Overlapping timelines
- [ ] Test animation + manual property updates
  - Animation should take priority
- [ ] Test nested element opacity (cumulative)
- [ ] Test transform composition with nested elements

**Performance Tests** (not yet implemented)
- [ ] Benchmark animation system overhead
- [ ] Test with 100+ concurrent animations
- [ ] Profile layout animation relayout frequency
- [ ] Measure memory usage of active animations

#### Demo & Examples

**Animation Demo Scene** (not yet implemented)
- [ ] Create `animation` demo scene in demo-app
  - Location: `demo-app/src/scenes/animation.rs`
- [ ] Showcase transitions on hover/focus
  - Button hover effects
  - Link hover colors
  - Container size changes
- [ ] Showcase keyframe animations
  - Continuous rotation
  - Bouncing elements
  - Color cycling
- [ ] Showcase easing functions
  - Side-by-side comparison of all easing curves
  - Visual timing diagrams
- [ ] Add to `cargo run -p demo-app -- --scene=animation`
- [ ] Create JSON IR examples for common animation patterns

#### Documentation

**Code Documentation** (partially complete)
- [ ] Document animation module architecture in `docs/animation.md`
  - System overview
  - Component relationships
  - Threading model
- [ ] Add animation examples to IR schema documentation
  - JSON examples for transitions
  - JSON examples for keyframe animations
  - Property reference table
- [ ] Document performance considerations
  - Layout vs visual property animations
  - When to use transitions vs keyframes
  - Animation batching strategies
- [ ] Add migration guide for existing IR documents
  - How to add animations to existing UIs
  - Common pitfalls and solutions
- [x] Update `CLAUDE.md` with animation-related information (partial)

### Low Priority - Optimizations

#### Performance Optimizations

**Animation System** (not yet implemented)
- [ ] Batch animation updates for same frame
  - Currently updates happen individually
  - Group updates by node or property type
- [ ] Skip rendering unchanged frames
  - Track if animation actually changed values
  - Avoid redraw when values haven't changed
- [ ] Use dirty flags for animated properties
  - Mark nodes as dirty when animations update
  - Only recompute affected subtrees
- [ ] Animation pooling
  - Reuse ActiveTransition objects
  - Reuse ActiveKeyframeAnimation objects
- [ ] Optimize property snapshot creation
  - Only snapshot animatable properties
  - Cache snapshots when possible

**Layout Optimization** (not yet implemented)
- [ ] Smart Taffy relayout triggering
  - Currently rebuilds on every frame with layout animations
  - Batch relayout requests
  - Skip relayout for visual-only properties
- [ ] Partial layout invalidation
  - Only relayout affected subtrees
  - Propagate layout changes efficiently
- [ ] Layout animation caching
  - Cache layout results between animation frames
  - Invalidate only when layout properties change

**GPU Optimization** (future consideration)
- [ ] GPU-accelerated transform animations
  - Offload transform matrix calculations
  - Use GPU transform pipeline
- [ ] GPU-accelerated opacity
  - Use GPU blend modes
  - Offscreen rendering for opacity layers
- [ ] Shader-based color interpolation
  - Interpolate colors in fragment shader
  - Reduce CPU overhead

### Future Enhancements - Advanced Features

#### Advanced Animation Features (stretch goals)

**CSS Animation Parity** (partially implemented)
- [ ] Support CSS shorthand syntax parsing
  - `transition: all 300ms ease` → TransitionSpec
  - `animation: spin 1s infinite` → KeyframeAnimation
- [ ] Support multiple concurrent animations per property
  - Currently one animation per property
  - Layer multiple animations
- [ ] Support animation delays in milliseconds and percentages
- [ ] Support negative delays (start mid-animation)

**Physics-Based Animations** (not yet implemented)
- [ ] Spring easing function
  - Add `Spring { stiffness, damping, mass }` to EasingFunction
  - Implement spring physics equations
  - Add spring presets (gentle, bouncy, stiff)
- [ ] Momentum-based animations
  - Preserve velocity across transitions
  - Natural motion physics

**Advanced Transform Features** (basic support exists)
- [ ] 3D transforms (limited by 2D renderer)
  - rotateX, rotateY, rotateZ
  - perspective transforms
- [ ] Transform origin from spec
  - Currently defaults to center
  - Support top-left, top-right, etc.
  - Support percentage-based origins
- [ ] Skew transforms
  - Already in transform types, needs rendering

**SVG/Path Animations** (not yet planned)
- [ ] Path morphing animations
- [ ] Stroke dasharray animations
- [ ] SVG filter animations

#### Developer Experience

**Debugging Tools** (not yet implemented)
- [ ] Animation timeline visualizer
  - Show all active animations
  - Visualize timing and easing
  - Pause/resume/step through
- [ ] Animation inspector in devtools
  - Real-time animation state
  - Property value scrubbing
  - Performance metrics
- [ ] Animation recording/playback
  - Record animation sequences
  - Export as video or GIF
  - Regression testing

**Animation Presets** (not yet implemented)
- [ ] Built-in animation library
  - Common UI transitions
  - Material Design animations
  - iOS-style animations
- [ ] Animation composition helpers
  - Sequence builder
  - Parallel animation groups
  - Stagger effects

---

## Implementation Status Summary

### ✅ Fully Implemented (Phases 1-9)
- Core animation infrastructure (types, easing, interpolation)
- Transition system (start, update, cancel, retarget)
- Keyframe animation system (multi-step, iterations, direction)
- Animation registry (named animations)
- IR schema integration (TransitionSpec, KeyframeAnimationSpec)
- Animation resolver (property value resolution)
- Transform support (2D matrix, origin, composition)
- Opacity & visibility support (values resolved)
- State change detection (PropertySnapshot, StateTracker)
- Animation events (Started, Ended, Cancelled, Iteration)
- Layout property animation (width, height, padding, margin)
- Background color animation (solid colors)
- Transform rendering (applied to canvas stack)
- Visibility rendering (Hidden/Visible/Collapsed)

### 🔶 Partially Implemented
- Opacity rendering (value resolved, canvas layers not implemented)
- Hover/focus state tracking (infrastructure exists, not wired to events)
- Border animations (types exist, rendering not implemented)
- Text property animations (types exist, rendering not implemented)

### ❌ Not Implemented
- Canvas opacity layer support
- Element-specific property animations (text, borders, buttons)
- Event handler wiring (hover, focus, active states)
- Unit and integration tests
- Demo scene and examples
- Performance optimizations
- Advanced features (physics, 3D, debugging tools)
- Comprehensive documentation

### 📊 Completion Metrics
- **Core Infrastructure**: 100% (Phases 1-4, 6-9)
- **Rendering Integration**: 60% (basic support done, full integration pending)
- **Element Integration**: 30% (layout + background colors done, text/borders pending)
- **Testing**: 0% (no tests yet)
- **Documentation**: 20% (implementation checklist exists, user docs pending)
- **Optimization**: 0% (no optimizations yet)

**Overall Completion**: ~50% of production-ready animation system

---

## Notes

- **Thread Safety**: AnimationManager should be `Send` for potential future async usage
- **Determinism**: Animations should produce identical results given same inputs (no RNG)
- **Frame Independence**: Use delta time, not frame count, for timing
- **Cancellation**: Always support clean cancellation without visual glitches
